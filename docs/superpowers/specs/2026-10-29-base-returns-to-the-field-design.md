# The base returns to the field — design

**Date:** 2026-10-29
**ADR:** [0032](../../adr/0032-base-returns-to-the-field.md) (Proposed — no code builds on this until the user agrees it)

## Intent

- The base sits on the map the player walks, and the player grows it outward tile by tile with Core Fragments.
- Mining opens the field itself. The blocking tiles that stop movement today become the rock a base digs into.
- The field and the base persist across breaches. A breach raises the danger in place instead of generating a new map.

## Today

- Since v0.13.0 the base lives in its own `BaseGrid` space (archived spec `2026-08-19-base-out-of-phase-design.md`).
  - The player enters it from a field `BaseAnchor`.
  - While inside, the player's field `Position` stays pinned to the anchor.
- Founding is free and lays a 19x9 pocket.
- Field blockers cannot be changed by anything in play. These are the cells where `Biome::walkable` is false: `DataVoid`, `BlackIce` and `Entropy`.
- Each breach generates a new field map.

## Decisions

1. **There is only the field.**
   - Base space is retired. That removes:
     - `enter_base` and `leave_base`
     - the anchor pin
     - `stands_in_base_space`
     - the `require_base`/`require_surface` split
   - Structures, staff, dig sites, farm tiles and besiegers become ordinary field bodies.
2. **Founding costs fragments and lays one ring.**
   - Placing the base anchor (the Home) costs Core Fragments. The cost is a constant in `tuning/base.rs` and must be covered by every class kit.
   - Founding floors exactly the 8 cells around the anchor, a 3x3.
   - Founding is refused unless all 8 ring cells are walkable, so the ring is never broken.
   - It is still refused on a Stack link.
3. **Growth is contiguous.**
   - A floor request costs `FLOOR_TILE_COST`.
   - It is refused (`PlacementRefusal::NotAdjacent`) unless the cell is orthogonally next to laid floor or a pending floor request. No gaps.
4. **Blockers are mined first.**
   - A floor request on a non-walkable field cell is refused (`PlacementRefusal::NeedsMining`). A floor mark never queues a dig on its own.
   - A blocker can be mined in two ways: a player bump with `MiningMode` on, or a crew dig mark.
   - Mining turns the cell into walkable ground, stored as a field tile override and saved.
5. **Fragments come from cheaper early tiers plus field rock.**
   - A mined field blocker drops a fragment at the existing chance (`BASE_MINE_FRAGMENT_CHANCE`, renamed for the field).
   - Rock kind (`assets/rock/`) is derived from the field biome, not a `BaseGrid` seed.
   - The Extractor and the other first-tier structures are re-costed so a class kit plus a few mined cells reach the first worker structure.
   - Final numbers are set in the plan and checked against `balance_sim` and the class kits.
6. **Sieges come from the base's edges.**
   - Besiegers spawn on walkable field cells orthogonally next to an edge floor tile.
   - Spawn cells are drawn from a list sorted by coordinate, never in query order.
   - Besiegers then path inward.
   - The `BASE_EXIT_CELL` flood fill and the saved `player_cell` go away.
7. **A breach re-tiers the field in place.**
   - The field map and the base persist; no new map is generated.
   - The zone number advances, and new wild entities spawn at the new zone's tier.
   - Dungeons the player has not entered migrate to the new zone's tier. Entered dungeons keep their tier.

## Rules kept, moved and deleted

- **Kept, now on field coordinates:**
  - the dig → floor → build loop
  - the `BuildSite` crew raise
  - `place_wall_box`
  - the dig-crew jobs
  - the floor and finish budgets in `drop_dry_dig_wants`
- **Placement conflicts:**
  - Floor is refused on a Stack link, a settlement footprint or an outpost cell.
  - Settlement placement avoids base floor.
  - `move_anchor_to`'s settlement-displacement caller is deleted.
- **Footprint distance replaces anchor distance:** the outpost minimum distance and `raiding_towns` measure to the nearest base floor, not the anchor.
- **Drawing:** one space, the field. Base floor draws through the existing `Biome::Platform` render path or its successor.
- **Deleted:**
  - `BASE_EXIT_CELL`
  - `lay_starting_pocket` and its pocket constants
  - `BaseGrid`'s seed and rock derivation
  - `Locale::Base`
- **Seams:**
  - `.claude/rules/seams-base.md` and `seams-sieges.md` are rewritten, along with their `seam:` graph entries and the seams skill references.
  - The order follows the seams skill.

## Save format

- **Version bump:** `save::SAVE_FORMAT_VERSION` goes up, which makes this a breaking minor release. Old saves stop loading, with no migration.
- **New saved state:**
  - floor cells in field coordinates
  - mined-cell overrides
  - a per-dungeon "entered" flag, if one is not already saved
- **Removed:** base-space positions and `player_cell`.
- **Tests:** each new field gets a save→load test, not only a RON round trip.
- **Templates:** every `dev-saves/` template is recaptured.

## Phases

Each phase lands green.

1. **Breach re-tiers in place.** This is independent of where the base lives. Read the `populated-chunks-clear-is-half-a-re-tier` memory first.
2. **Field mining.** Blockers become diggable, with fragment drops and saved overrides.
3. **The base onto the field.**
   - `BaseGrid` becomes the field-coordinate floor overlay.
   - Base space, the pin and the require split are deleted.
   - Sieges are reworked (decision 6).
   - The placement conflicts are added.
4. **Founding and growth.**
   - The 3x3 ring.
   - The `NotAdjacent` and `NeedsMining` refusals.
   - The anchor cost and the re-costed early tiers, with `balance_sim`.
5. **Release.** The save format bump, the dev-save recapture and the CHANGELOG entry.

## Open questions

Resolve these in the plan or ask the user.

1. Can mining happen anywhere on the field, or only within reach of the base?
2. What does a mined cell become: the zone's dominant walkable biome, or a fixed "cleared" biome?
3. On a re-tier, what happens to Stack links, settlements, outposts and nests?
4. A perimeter with no walkable edge cell (fully walled, or bounded by rock): does the siege skip, or do besiegers dig?
5. Does a pending floor request count as adjacency? This spec says yes, so a line of tiles can be queued in one go.

## Testing

- **Engine unit tests for each decision:**
  - The ring founding, and its refusal when the ring is blocked.
  - `NotAdjacent` and `NeedsMining`.
  - A mined cell persists through save→load.
  - Siege spawn cells lie on the edge and are deterministic.
  - A breach keeps the map and the floor, re-tiers an unentered dungeon and leaves an entered one alone.
- `cargo test -p feral-processes-engine balance_sim` after the re-cost.
- Gates: `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- A `--screenshot` of a founded 3x3 base from a recaptured template.
