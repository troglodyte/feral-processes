# Zone Currency Reset — design

**Status:** approved design, not yet implemented
**Date:** 2026-07-25

## Problem

Breaching into a new zone carries the player's whole fragment stockpile
forward. You can farm zone 1 until you are rich, then chain-breach on
banked currency without ever engaging with the zone you are standing in.
The intent is the opposite: each zone should have to fund its own exit.

The per-zone cost multiplier on the Portal (`structure_build_cost`,
lib.rs:6124 — `qty * zone`) was the existing counterweight, but it only
taxes a stockpile; it doesn't require the stockpile to be *earned here*.

## Goals

- Arriving in a new zone means arriving broke. Portal Fragments and Core
  Fragments for the next breach must be acquired in the zone you are
  leaving from.
- Nothing that represents accumulated *progress* is lost — gear, fusion
  tiers, research, companions, and the base all survive as they do today.

## Non-goals

- Changing what survives a breach beyond currency (structures, tamed
  programs, and the base platform keep travelling; see
  `2026-07-24-travelling-base-design.md`).
- Reworking drop rates, node yields, or the Market.
- Any new item, structure, or per-zone tagging of stacks.

---

## 1. The wipe rule

In `Game::enter_next_zone` (`crates/engine/src/lib.rs:4446`), after the
travellers are repositioned, drop every `Inventory` stack whose
`ItemDef.role` is `EconomyRole::Currency` or `EconomyRole::CraftCurrency`.
Capture each stack's count as it is removed — the log line below needs the
amounts, and they are gone by the time it runs.

Shipped, exactly three items carry a role at all:

| Item | Role | On breach |
|---|---|---|
| `core_fragment` | `Currency` | **wiped** |
| `portal_fragment` | `CraftCurrency` | **wiped** |
| `research_data` | `ResearchCurrency` | kept (banked, cap 200) |

Everything else — Power Cells, ICE Breakers, equippable gear, uncrafted
drops — has no role and survives. This is an economic reset, not a supply
confiscation: the player keeps what they are carrying and loses what they
are saving.

The rule keys on `EconomyRole`, never on the ids `"portal_fragment"` /
`"core_fragment"`. A mod that ships its own currency item gets the
behaviour with no engine change, which is the same discipline
`Game::craft_currency` and `Game::currency` already follow (lib.rs:621).

`Inventory` is a player-only component — spawned at lib.rs:825, restored
from a save at lib.rs:818 — so there is no second holder to sweep. Worked
nodes pay their yield straight into `Inventory` on cycle completion
(`systems.rs:158`; `amount` is a refill counter, not a stash), so no
buffered output survives the breach either.

### Log line

A second `self.log` after the existing breach message, emitted **only when
something was actually lost**:

```
Your fragment caches decohere in transit — 12 Core Fragments and
7 Portal Fragments lost.
```

Without it the wipe reads as a bug. Name the items via `Game::item_name`
so a mod's currency names itself correctly; list only the roles that had a
non-zero count.

## 2. Softer portal cost ramp

The wipe converts the Portal's cost from a tax on a stockpile into a fresh
grind every zone. Keeping `qty * zone` on top of that would double-dip: at
zone 5, 50 fragments from zero is ~143 kills at the 35%
`PORTAL_FRAGMENT_DROP_CHANCE`, against programs with 16× base stats.

Replace the multiplier in `Game::structure_build_cost` with percentage
growth off the base rate:

```
qty + qty * ZONE_PORTAL_COST_GROWTH_PERCENT * (zone - 1) / 100
```

`ZONE_PORTAL_COST_GROWTH_PERCENT: u32 = 50`, sited next to
`PORTAL_FRAGMENT_DROP_CHANCE` (lib.rs:229) with the other economy
constants. (`balance.rs` is a projection module, not a constants file —
the CLAUDE.md pointer to it for named constants is stale.)

For the shipped 10-fragment Portal:

| Zone | Today | New |
|---|---|---|
| 1 | 10 | 10 |
| 2 | 20 | 15 |
| 3 | 30 | 20 |
| 4 | 40 | 25 |
| 5 | 50 | 30 |

Expressed as a percentage of the base rate rather than a literal
`10 + 5 * (zone - 1)` so it stays correct for a modded Portal priced at
something other than 10, or priced in more than one item. Integer division
last, so the shipped numbers are exact.

Non-portal structures are untouched: the `def.zone_portal` branch is the
only place the multiplier applies, as today.

## 3. Keeping the balance projection honest

`balance::ticks_to_afford_portal` (balance.rs:137) hardcodes
`portal_fragment_rate * zone`. Left alone it would project a cost the game
no longer charges.

Extract the cost math into one function both call sites use:

```rust
pub fn zone_portal_cost(base_qty: u32, zone: u32) -> u32
```

in `balance.rs`, called by `Game::structure_build_cost` and by
`ticks_to_afford_portal`. Its doc comment ("A Portal's build_cost is a
per-zone-level rate") needs rewriting to describe the growth rate.

The sim measures ticks-to-afford *from zero*, which the wipe now makes
literally true rather than pessimistic. Its guard test
(`a_tiered_base_funds_deeper_portals_faster_than_a_fresh_one_funds_shallow_ones`)
should still hold — cost now grows more slowly while node payout still
doubles per zone — but run it rather than assume it.

## 4. Save format

No version bump. `Inventory` serialises as a `Vec<(ItemId, u32)>` either
way; the wipe is state, not schema.

## 5. Testing

New tests in the `engine` test module:

- Breaching zeroes the `Currency` and `CraftCurrency` stacks. Grant both,
  `enter_next_zone`, assert `count` is 0 for each.
- Breaching keeps everything else: `research_data`, an equipped item, an
  unequipped gear stack, Power Cells, and `ItemFusions` tiers all survive.
- The wipe log fires only when something was lost — breach with an empty
  wallet and assert no decohere message.
- `structure_build_cost` for the Portal is 10 / 15 / 30 at zones 1 / 2 / 5.

One existing test needs rewriting:
`portal_build_cost_scales_with_current_zone_level` (lib.rs:15812) asserts
the old ×zone numbers — it tops up to 19 expecting failure and to 20
expecting success at zone 2. Under the new formula zone 2 costs 15, and
the top-up now happens *after* a wipe rather than on a residue of 0 that
happened to coincide. Rewrite both arms and the assertion messages.

The other five
`enter_next_zone` tests (lib.rs:15584–15711) assert on structures,
durability, node stock, cronjob targets, and companions — none reads a
currency count after the breach.

`test_assets_dir()` resolves to the real `assets/` tree (lib.rs:6448), so
the role tags on the three currency items are live in tests — no fixture
needs updating.

## 6. Documentation

- `assets/structures/README.md` — the `zone_portal` comment block
  (lines 91–95) documents the field's cost behaviour; it must state the
  growth rate and the currency wipe.
- `Game::describe_structure` (lib.rs:6019) says "breaches to the next
  zone; cost scales with zone level". Still true, but it should say what
  the player loses. The assertion at lib.rs:7910 only checks for
  "next zone", so the wording is free to change.
- Root `README.md` / `CHANGELOG.md` — grep for claims about carrying
  resources between zones that this falsifies.
