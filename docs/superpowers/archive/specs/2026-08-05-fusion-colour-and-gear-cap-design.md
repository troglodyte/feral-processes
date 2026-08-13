# Fusion colour in menus, and a 3-fuse ceiling on gear

2026-08-05

Two changes that share one idea: fusion depth is a permanent property of a
thing you own, it should read at a glance, and it should mean the same
number of fusions whether the thing is a program or a piece of gear.

## Where this starts

- A **program**'s fusion depth lives in `components::FusionCount`, is capped
  at `tuning::MAX_FUSIONS` (3), and reads as a text tag —
  `(fused 2/3)`, `(fused 3/3 - maxed)` — built by `render/party.rs::fusion_tag`.
- A **piece of gear**'s fusion depth lives in `components::ItemFusions`,
  keyed per `ItemId` on the player, is **uncapped**, and adds
  `ITEM_FUSION_BONUS_PER_TIER` (+10%) per tier with a
  `ITEM_FUSION_MIN_BONUS_PER_TIER` (+1) flat floor. It reads as a terse
  `T2` note in the equipped panel and the swap picker's stat column, and as
  `fusion T2` inside `equip_preview_tag`.
- Neither is coloured anywhere.

## 1. One vocabulary, two subjects

`fusion_tag` moves from `render/party.rs` to `render/mod.rs`, beside
`hp_critical` and `activity_tag` — the file that already holds the row
vocabulary shared across screens. A sibling joins it:

```
fusion_color(fusions: u32) -> Option<Color>
    0        → None       (plain TEXT, unchanged)
    1..MAX   → CYAN
    >= MAX   → MAGENTA
```

Programs and gear call **the same function**; gear passes its `ItemFusions`
tier. That is what makes "gear caps like programs do" literal rather than a
second parallel implementation free to drift. `Option<Color>` rather than a
defaulted `Color` so a caller that already has a colour rule of its own
(the party screen's CRITICAL red) composes with it instead of being
overwritten.

Two colours rather than one because the colour then carries exactly what the
text tag carries: cyan means *fused and still fusable*, magenta means *at the
ceiling, no longer an input*. On a fuse picker that is the whole decision.

## 2. Where the colour lands

Row colour is already a cheap seam: `render/popup.rs::colored_item_row`
exists and both fusion counts are already reachable from the renderer
(`PetInfo.fusions`, `Game::item_fusion_tier`).

**Programs** — every menu that lists them:

| Screen | Site |
|---|---|
| Party (`p`) | `render/party.rs::draw_companion_menu` |
| Fuse picker, first and second | `render/party.rs::draw_fuse_menu`, `draw_fuse_second_menu` |
| Extract picker (`M`) | `render/routines.rs::draw_extract` |
| Cronjob / worker picker | `render/building.rs` — already cross-refs `owned_pets` for PWR |
| Trade → "Sell programs (permanent)" | `render/trade.rs` |

The trade row is the one that needs an engine change: `views::ProgramSaleOption`
carries no fusion count, so it gains `fusions: u32`. That screen erases a
program permanently and is precisely where a 3/3 wants to be loud.

**Gear** — every menu that lists an owned item:

| Screen | Site |
|---|---|
| Inventory list, and the three equipped rows | `render/inventory.rs::draw_inventory`, `equipped_row` |
| Equip swap picker | `app-core::SwapRow` gains `fusion_tier: u32`; `render/inventory.rs::draw_equip_swap` colours from it |
| Trade → "Sell (from inventory)" | `render/trade.rs` |
| Base pane's inventory listing | `render/base.rs` |

The swap picker goes through `SwapRow` rather than the renderer re-deriving
the tier, because app-core owns that screen's rows and gui only draws them —
a renderer that recomputed the tier could disagree with the label app-core
built beside it.

### Left uncoloured, deliberately

Each of these already spends its colour axis on something else, and a second
meaning on the same axis makes both unreadable:

- **Battle roster** (`render/battle.rs`) — row colour is HP state: red
  critical, cyan, green.
- **Manifest / `B` roster** (`render/manifest.rs`) — colour is the entity's
  own glyph colour, plus red for a boss. Fusion depth is already a text tag
  there (`fused 2/3`) and stays one.
- **Trade's Buy and Buy-back lists** — both pass fusion tier 0 on purpose,
  with the reason recorded in `trade.rs`: the stock is not yours and carries
  no tier of yours.
- **Recipe rows and build-cost lines** — a row about a recipe, not about an
  owned item.

### The one collision

On the party screen a row can be both fused and CRITICAL. Red wins: critical
is an urgent, transient state the player must act on this turn; fusion is a
permanent property they can read at leisure. This is why `fusion_color`
returns `Option` — `draw_companion_menu` checks `hp_critical` first and only
falls through to the fusion colour.

## 3. The gear ceiling

`Game::fuse_item` (`game/crafting.rs`) gains an early refusal mirroring
`fuse_companions`:

> `{name} has already been fused 3 times — it can't be fused again.`

placed **before** the `Inventory::take`. Same ordering rule `install_routine`
and `use_symlink` follow, and for the same reason: a refused action must not
have spent anything. `fuse_item` already has a partial-take rollback for the
insufficient-stock case; the ceiling check sits above it so no rollback is
needed at all.

Gear reuses `tuning::MAX_FUSIONS` rather than getting its own constant. A
separate `MAX_ITEM_FUSIONS` would be a second knob nothing currently wants to
turn independently, and the point of the change is that the two ceilings are
*the same rule*.

`ItemFusions::increment` stays dumb — it does not silently saturate. The
refusal in `fuse_item` is the gate; a second quiet clamp inside the component
would hide a caller that skipped the gate rather than surfacing it.

### How the ceiling reads

`app-core::equip_preview_tag` is the one place three screens get their item
tag from (inventory list, item-action page, trade sell row). It goes from
`fusion T2` to `fusion T2/3`, and to `fusion T3/3 - maxed` at the ceiling —
the same shape a program's `(fused 3/3 - maxed)` has.

The two compact notes — `render/inventory.rs::equipped_summary`'s `T1` and
`app-core::equip_swap_rows`' `T{tier}` — show the `/3` fraction but not the
word "maxed". `SWAP_STATS_COLUMN` is 20 monospace cells and
`+2 ATK +1 DEF T3/3 maxed` is 24; the colour carries "maxed" in those two
columns instead.

All three sites take the fraction from one new `pub fn item_fusion_note`
in `app-core/src/lib.rs`, beside `equip_preview_tag` and `stat_summary` —
app-core because two of the three callers already live there, and one
function because the alternative is three literals of `/{MAX_FUSIONS}` that
a later retune of the constant would leave disagreeing. `equip_preview_tag`
appends the " - maxed" wording on top of it.

## 4. Clamping a legacy save, and the constraint on it

A save can hold an item above tier 3 — the mechanic was uncapped, and
`savetool` can hand-edit one in either way. The ceiling should be true
everywhere, so `ItemFusions.tiers` is clamped to `MAX_FUSIONS` where
`game/lifecycle.rs` builds the component from `PlayerSave::item_fusions`.

**The worn copies are not clamped, and that is load-bearing.** Saved `Stats`
already *include* the equipment bonus: `apply_equipment_delta` writes
straight into `Stats`, and the load path restores those numbers verbatim
(`game/lifecycle.rs`, the `Stats { .. }` in the player bundle).
`EquippedItem::fusion_tier` is only the record of what was added. Clamping
`weapon_fusion_tier` on load would therefore break the add/subtract pair: the
bonus went on at tier 5, unequipping would take off tier 3, and the
difference would be welded permanently into the player's base stats — an
invisible buff produced by a change whose entire purpose was a nerf.

So a legacy worn copy keeps its old bonus until it comes off, and picks up
the clamped tier when re-equipped. The ledger governs every *future* equip
and every future fusion, which is what the cap is actually about.

No `SAVE_FORMAT_VERSION` bump: no field's shape or meaning changes, and an
older save loads into a legal state.

## 5. Balance note

`MAX_FUSIONS` at `ITEM_FUSION_BONUS_PER_TIER` caps gear at **+30%**, where it
was previously unbounded. Every shipped equippable sits in the 1..=4 stat
range, where `ITEM_FUSION_MIN_BONUS_PER_TIER` floors a tier at +1 flat — so
the effective ceiling is about +3 to a stat.

This is a real nerf to a player stacking one item type, and it is accepted
without compensation: an unbounded multiplier on a repeatable action was the
thing worth removing, and `balance_sim`'s `best_case_gear_bonus` models gear
*without* fusion, so the shipped curves do not move. `cargo test -p
feral-processes-engine balance_sim` is still run as the gate, because it is
the gate for any change that touches an item's effective stats.

## 6. Tests

Failing-first, in this order:

1. **Engine** — `fuse_item` at `MAX_FUSIONS` returns `Err` *and leaves the
   stack untouched*. Asserting the inventory count is the point: it is what
   pins the check above the take rather than below it.
2. **Engine** — a `PlayerSave` carrying an item at tier 5 loads with
   `ItemFusions` at 3, while the worn copy's `fusion_tier` and the player's
   `Stats` are unchanged. This is section 4's constraint, pinned so a later
   tidy-up cannot "finish the job" by clamping the worn tier too.
3. **Engine** — `ProgramSaleOption` carries the program's fusion count.
4. **App-core** — `SwapRow` carries the tier; `equip_preview_tag` prints the
   fraction, and the maxed note at the ceiling.
5. **Gui** — `fusion_color` over 0, 1, 2, 3, 4.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`, and
`cargo test -p feral-processes-engine balance_sim`.

## 7. Docs

- `docs/manual.md` is **not** updated — carved out of the doc obligation as
  of 2026-08-04 until that carve-out is lifted. Note that its fusion section
  currently describes gear fusion as unbounded, so it becomes stale here on
  purpose.
- `README.md` and `CHANGELOG.md` get the cap and the colour, per standing
  practice.
- No `assets/*/README.md` change: no schema field is added, removed, or
  changed in meaning.

## Out of scope

- Compensating the nerf by retuning `ITEM_FUSION_BONUS_PER_TIER`.
- Colouring the battle roster or the manifest.
- Per-tier colour ramps.
- Making gear fusion per-stack rather than per-`ItemId`.
