# Settlement footprints

**Status:** approved design, not yet implemented.

## Intent

A town (`SettlementKind::Server`) or city (`Mainframe`) takes up a square of
surface tiles sized by what it is now, instead of one tile. Size follows kind
*and* a city's `growth::Vitality`, so a thriving city sprawls and a starving
one shrinks. Anything standing where a footprint grows is displaced. Nobody
walks in: every footprint cell is the door, as the one tile is today.

## Size

Derived on every read, never stored — `growth::vitality`'s rule, so a retune
re-sizes every existing save and there is no save-format change.

| State | Radius | Square |
|---|---|---|
| Server | 1 | 3×3 |
| Mainframe, Starved | 1 | 3×3 |
| Mainframe, Steady | 2 | 5×5 |
| Mainframe, Thriving | 3 | 7×7 |

- `Game::settlement_radius(key) -> i32` is the one derivation, over
  `settlement_kind` and `settlement_vitality`. It is an exhaustive match, so a
  new kind or band fails to compile rather than defaulting.
- The three radii are `pub const`s in `tuning.rs`, plus
  `SETTLEMENT_FOOTPRINT_MAX_RADIUS` (the largest of them), which the outpost
  gate reads.
- A new city reads Steady, because `vitality_floor` holds an untraded city
  there.
- `REGION_EDGE_INSET` (24) keeps two footprints from ever touching.
- A footprint covers every cell whatever the terrain underneath. Blocking
  terrain is simply built over.

## Map entities

- Every footprint cell is its own entity carrying `components::Settlement
  { key }`, a `Position` and the kind's `Glyph`. The repeated glyph is the
  look the user asked for, and the renderer, examine
  (`drawn_on_surface_map`, `find_target_in_direction`), `find_settlement_at`,
  `bump_tiles` and the trap placement gate all keep working with no change.
- **The centre cell also carries a new `components::SettlementCentre`
  marker.** `TownPatrol { town: Entity }` tethers to a town *entity*, and the
  outer cells are despawned when a footprint shrinks. `settlement_entity`
  (`settlement_patrol.rs`) filters on the marker, and the centre always exists
  because the radius is never below 1. The load path's `towns_by_tile`
  resolves a patrol by the town's tile, which is the centre, so it resolves
  to the same entity.
- **`Game::sync_settlement_footprint(key)` is the one writer of those
  entities.** It diffs the cells that should exist against the cells that do,
  spawns and despawns to match, repaints every cell's glyph to the current
  kind, and displaces from every newly covered cell (below). It is called:
  - by `spawn_settlement_at`, when a town materializes or `restore_settlements`
    rebuilds it;
  - by `announce_growth`, which replaces its own single-entity repaint;
  - once a tick for every known settlement, from the tick that already runs
    `settlement_growth_tick`, because vitality moves with commerce at any
    time. It returns early when the radius and glyph are unchanged.

## Displacement

This runs on every cell a footprint newly covers, including at first
materialization, since `standable_near` only ever checked the centre. The
target is the nearest free tile outside the footprint, found by one helper
built from `relay_landing`'s search: `ring_tiles`, walkable, and no wild
creature, nest, surface link, settlement cell, trap or outpost.

| Occupant | Handling |
|---|---|
| Wild creature, trap | `Position` rewritten. |
| Caravan | `Position` and `arrival_tile` both rewritten (see Knock-on fixes). |
| Nest | `Position` rewritten. Guardians follow by entity, and `NestSave` already writes the nest's current tile. |
| Player on the surface | `Position` rewritten. |
| Stack entrance | Relocated the way `collapse_stack` already does it: despawn, drop that entrance's `StackMemory` entries, spawn at the new tile. The seed follows the entrance tile, so it becomes a new maze. **Deferred while the party is inside that Stack** (`Locale::Stack { entrance }` equals it) and applied when they surface, because the player's `Position` is pinned to the entrance while underground. |
| Base anchor | `Game::move_anchor_to`, its one writer. Base space itself is untouched. |
| Outpost | Never displaced. `found_outpost` refuses a site within `SETTLEMENT_FOOTPRINT_MAX_RADIUS` of a known town's tile, so a growing city cannot reach one. |

A displacement is silent, except for a Stack entrance, which logs one line
because what the party had seen of that Stack is lost.

## Knock-on fixes

- **Trading reach** (`settlement_market.rs:79`, currently within one tile of
  the centre). This becomes "next to the footprint":
  `chebyshev(pos, tile) <= radius + 1`.
- **Relay landing** (`relay_landing`). Its search starts at band `radius + 1`
  instead of 1.
- **Spawn exclusion.** Wild spawning (`populate_chunk`, `spawn_wild_nearby`,
  `try_spawn_habitat_creature`, and so nest placement), `link_site_free`,
  `patrol_stand` and `scatter_open_tile` skip cells where
  `find_settlement_at` answers.
- **Unchanged.** Aid radii, patrol leashes, raids, routes and the compass
  keep measuring from the centre. At most 3 tiles of difference doesn't
  matter against radii that are fractions of `REGION_TILES`.
- **Caravans.** A caravan spawns up to `CARAVAN_SPAWN_DISTANCE_TILES` from
  the anchor, along a bearing, on the first *walkable* tile, then walks back
  to that `arrival_tile` when it leaves (`caravan.rs:808`). The spawn search
  also skips footprint cells. A caravan standing where a footprint grows is
  displaced like a wild creature, **and its `arrival_tile` is rewritten to
  the new tile**, or its walk home would aim into the town.

## Seams

"The tile admits nobody" becomes "the footprint admits nobody". One new
seam gets all three writes (CLAUDE.md rule, `seams` skill trap, graph
argument):

- The footprint is derived and `sync_settlement_footprint` is its one writer.
- The centre is the entity a tether names.

## Testing

All tests are engine-side and seeded, and each one is mutation-checked.

- **Radius table.** One test per row.
- **Materialization.**
  - A Server spawns 9 cells with one centre.
  - Bumping a corner cell queues the visit and does not move the player.
- **Growth.** Latching growth moves a town from 9 cells to 25 and repaints
  them all to `M`.
- **Vitality.** A Thriving city has 49 cells, and pushing it to Starved takes
  it back to 9.
- **Displacement, one test per occupant.**
  - A wild creature, trap, nest (with its guardian still tethered) and the
    player each end up outside the footprint.
  - A Stack entrance is relocated and its `StackMemory` dropped.
  - The same entrance is **not** relocated while the party is inside, and is
    relocated on surfacing.
  - The anchor moves.
- **Outposts.** Founding at radius 3 from a town is refused.
- **Trading reach.** The player next to a corner of a 5×5 city can trade.
- **Patrols.**
  - A patrol stays tethered through a shrink.
  - A save → load round trip re-tethers it.
- **Spawning.** Wild creatures never spawn inside a footprint, with the RNG
  seeded.
- **Save/load.** The footprint rebuilds at its derived size.

## Out of scope

- Streets, or walking inside a town.
- Sprite art for town cells.
- A per-settlement size authored in `.ron`.
