# Inventory capacity

## Problem

The player's inventory is unbounded. Nothing forces a choice about what to
carry, so resource pressure comes only from acquisition rate, never from
storage. At the same time the Data Cache is dead content: it costs 15 Core
Fragments, sits behind a 20-point research node, and has no implementation
anywhere in the engine outside a unit test fixture.

Both problems have the same answer: cap the inventory, and make the Data
Cache the thing that raises the cap.

## Design

### Capacity is derived, never stored

```
capacity = BASE_INVENTORY_CAPACITY (20) + Σ inventory_bonus over deployed structures
used     = Σ quantities in Inventory
```

`used` counts total units, not distinct stacks. A cap on stacks could never
bind — only 11 `ItemId` variants exist.

Capacity is recomputed every time it's read. Nothing caches it, so a Data
Cache destroyed by a raid shrinks the buffer with no invalidation step, and
the save format does not change.

Equipped gear does not count. `Game::equip` already removes the item from
`Inventory`, so a worn Monofilament Whip occupies no buffer — which is why
unequipping has to be gated rather than clamped.

### Research Data is banked, not carried

Research Data is a currency, not cargo. Sharing the inventory cap would mean
a Research Node cronjob silently burning its own output whenever the buffer
filled with something unrelated — a structure sabotaged by resources it has
nothing to do with.

So it is exempt from inventory capacity and given its own ceiling instead:
`RESEARCH_DATA_BANK_LIMIT = 200`. Overflow clamps and logs, matching the rule
for every other unsolicited income path.

200 is chosen against a full remaining tree cost of 275 (the 12 nodes after
Cold Storage is deleted). The bank therefore cannot hold the whole tree at
once — research has to be spent along the way rather than hoarded to the end
— while still being generous enough that no single node, nor any realistic
prerequisite chain, is ever out of reach.

It stays an `ItemId` in `Inventory` rather than moving to its own component.
The Research Node produces it through the generic data-driven cronjob path
(`work: Some((produces: ResearchData, ...))` → `inv.add(node.resource, 1)`),
so extracting it would force a hardcoded Rust special-case for one structure
— exactly what CLAUDE.md's moddability rule forbids.

Instead `items.rs` gains one method alongside the existing `display_name()`
and `equipment()`:

```rust
/// `Some(ceiling)` for a banked currency — exempt from the shared
/// inventory capacity, limited only by its own hard cap. `None` for
/// ordinary cargo, which counts against `Game::inventory_capacity`.
pub fn bank_limit(self) -> Option<u32> {
    match self {
        ItemId::ResearchData => Some(RESEARCH_DATA_BANK_LIMIT),
        _ => None,
    }
}
```

One method answers both questions: `bank_limit().is_none()` is exactly "counts
toward inventory capacity", so there's no second predicate to keep in sync
with it. `inventory_used` sums only items where it returns `None`; every add
path clamps a banked item to its own limit instead.

### Capacity comes from structure data, not a hardcoded ID

`StructureDef` gains:

```rust
/// How much this structure raises the player's inventory capacity while
/// it's deployed (see `Game::inventory_capacity`). Stacks additively
/// across every deployed structure that sets it. `#[serde(default)]` so
/// existing structure files (including mods) contribute nothing.
#[serde(default)]
pub inventory_bonus: u32,
```

`data_cache.ron` sets `inventory_bonus: 10`. No Rust code mentions
`"data_cache"` — per CLAUDE.md, a mod can grant buffer space by setting the
field. `assets/structures/README.md` documents it in the same change.

Multiple Data Caches stack: each deployed one adds 10.

### Overflow: clamp for income, refuse for purchases

| Path | Behavior |
|---|---|
| Scan `g`, battle loot, boss fragment cache, cronjob output | **Clamp + log** — take what fits, log `"Buffer full — 3 Core Fragments lost."` |
| Research Node cronjob at 200 banked | **Clamp + log** — `"Research bank full — 1 Research Data lost."` |
| Compile `c`, buy at iso Market | **Refuse** — `"Buffer full."`, nothing consumed |
| Unequip | **Refuse** — `"Buffer full."` |

The split is deliberate. Clamping is right for unsolicited income, which must
never stall a cronjob worker or block a battle from resolving. Refusing is
right where the player pays an input cost: clamping a compile would consume
Core Fragments for an ICE Breaker that evaporates, and clamping an unequip
would silently delete a Monofilament Whip.

Mechanically:

- `Inventory::add_capped(item, qty, capacity) -> u32` returns how much
  actually landed. Clamping callers log the difference. It routes on
  `item.bank_limit()`: a banked item is checked against its own ceiling and
  ignores `capacity` entirely; cargo is checked against `capacity`.
- Refusing callers check `used + qty <= capacity` up front and bail before
  consuming anything.
- `Inventory::add` stays, uncapped, for save loading.

### Erasing a partial stack

`Game::erase_item(item, qty)` already accepts a quantity, but app-core always
passes the whole stack (`crates/app-core/src/lib.rs:1499`). Under a hard cap
that is actively hostile: freeing 2 units of room would force dumping all 18
Core Fragments.

Add `Mode::EraseQuantity` and `erase_quantity_input`, mirroring the existing
`CraftQuantity` / `TradeQuantity` modes. `[X]` on an inventory item opens:

```
Erase how many Core Fragment?
Quantity: 5
You have: 18        Buffer: 20/20

Type digits, Enter to erase
[A] Erase all   Esc to go back
```

`[A]` preserves today's erase-everything behavior. `Game::erase_item` needs
no change; this is an app-core mode plus the two render functions.

### Data Cache cost and availability

Build cost drops from 15 to 10 Core Fragments. At base capacity 20 this
leaves room to hold a cache's cost alongside 10 units of other gear, so
expanding the buffer never requires emptying it first — at 15 the squeeze
was uncomfortably close to chicken-and-egg.

The Data Cache also becomes buildable from turn one. Gating the only means
of buffer expansion behind ~32 banked Research Data would lock the player at
capacity 20 through the entire early game, which is where the cap bites
hardest and where the player has the fewest tools to respond to it.

This is done by **deleting `assets/research/cold_storage.ron`**. Per that
directory's README, a structure named by no research file is buildable
immediately, so removing the file is the whole implementation. Nothing else
depends on the node — `unlocks_structures: ["data_cache"]` was its only
content, and no other node lists it as a prerequisite — so it would
otherwise survive as an empty 20-point research sink that unlocks nothing.
Its prerequisite, Isometric Commerce, still stands on its own by unlocking
the iso Market.

## Consequences

- **Old saves may exceed the cap.** A v5 save holding 200 Core Fragments
  loads fine and simply cannot gain more until spent. Over-cap is a legal
  state; only *adding* is gated.
- **No existing structure becomes unbuildable.** The priciest are Armory and
  Fabricator at 18 Core Fragments, under the base 20.
- **The Zone Portal is the cap's real pressure.** Cost is 10 Portal Fragments
  × zone level, so zone 3 needs 30 (1 cache), zone 5 needs 50 (3 caches),
  zone 10 needs 100 (8 caches). This is the intended gate, not a bug.
- **The research tree loses a node**, going from 13 to 12. Cold Storage is
  deleted rather than repurposed; if a future feature wants a storage-themed
  research node, it can add one with real content.
- **A Research Node cronjob eventually wastes its output.** Once the bank
  hits 200 the worker keeps running and keeps losing units to the log. That
  is the intended signal to go spend research, but it means an unattended
  Research Node is no longer strictly free value.
- **The bank cannot fund the whole tree.** 200 against 275 total means at
  least one spend is forced mid-tree; no ordering of research lets a player
  bank once and buy everything.
- **Erasing costs a tick.** `erase_item` calls `tick()`. Unchanged, but the
  cap means players will do it far more often.

## Implementation surface

- `crates/engine/src/structures.rs` — `inventory_bonus` field; free function
  computing capacity from deployed kinds + `StructureDb`, so `Game` and
  `task_progress_system` share one implementation.
- `crates/engine/src/components.rs` — `Inventory::add_capped`.
- `crates/engine/src/items.rs` — `ItemId::bank_limit`,
  `RESEARCH_DATA_BANK_LIMIT`.
- `crates/engine/src/lib.rs` — `BASE_INVENTORY_CAPACITY`,
  `Game::inventory_capacity()`, `Game::inventory_used()`; clamp/refuse at
  each add site.
- `crates/engine/src/systems.rs` — `task_progress_system` gains
  `Query<&Structure>` + `Res<StructureDb>` to compute capacity.
- `crates/app-core/src/lib.rs` — `Mode::EraseQuantity`.
- `crates/tui/src/ui.rs`, `crates/gui/src/render.rs` — `Buffer 11/20` in the
  inventory screen; erase-quantity prompt; the research menu's existing
  `Research Data: 87` line becomes `Research Data: 87/200` so the ceiling is
  visible before a cronjob starts wasting output.
- `assets/structures/data_cache.ron` — cost 10, `inventory_bonus: 10`.
- `assets/structures/README.md` — document `inventory_bonus`.
- `assets/research/cold_storage.ron` — deleted.

## Testing

Engine unit tests:

- capacity is 20 bare, 30 with one Data Cache, 40 with two
- a raid-destroyed Data Cache shrinks capacity back
- a clamped pickup lands the partial amount and logs the loss
- compile and buy refuse at a full buffer, consuming nothing
- unequip refuses at a full buffer, gear stays equipped
- a save loaded over cap cannot gain items but is otherwise playable
- Research Data does not count toward `used`, and a Research Node cronjob
  keeps producing it at a full buffer
- the Research Data bank clamps at 200 and logs the loss; a cronjob running
  at 200 banked stays assigned and does not error
- the Data Cache appears in `buildable_structure_defs` on a fresh game with
  no research completed

App-core tests: `[X]` opens `EraseQuantity`; a digit entry erases exactly
that many; `[A]` erases the stack.

Full `cargo test --workspace` is the gate.
