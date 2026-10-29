# 0032 — The base returns to the field; a breach re-tiers the field in place

- Status: Proposed
- Decided by: user, on Claude's proposal (awaiting the user's OK on this record)
- Date: 2026-10-29
- Pattern: —
- Spec: [2026-10-29-base-returns-to-the-field-design.md](../superpowers/specs/2026-10-29-base-returns-to-the-field-design.md)

## Context

Since v0.13.0 the base lives in its own space.

- `BaseGrid` is a sparse map of cells with its own seed. A cell with no entry is rock.
- The player enters it from a field `BaseAnchor`, through `Game::enter_base`, and the player's field `Position` stays pinned to the anchor while inside.
- Founding lays a free 19x9 pocket around `BASE_EXIT_CELL`.
- Each breach generates a new field map. The base survives only because it is off the field.

The move was recorded in an archived spec, `2026-08-19-base-out-of-phase-design.md`, not in an ADR.

What two spaces cost today:

- **Branching:** about 11 call sites choose between `require_base` and `require_surface`.
- **Rendering:** a one-space rule, `Game::stands_in_base_space`.
- **Sieges:** a flood fill from `BASE_EXIT_CELL` and a saved `player_cell`, both of which exist because of the pin.
- **Field terrain:** field blockers (`Biome::walkable` false) can never be changed by anything in play.

## Decision

- **Base space is retired.**
  - `BaseGrid` becomes a floor overlay keyed by field coordinates.
  - Structures, staff, dig sites, farm tiles and besiegers are ordinary field bodies.
- **Founding:** the anchor costs Core Fragments and floors exactly one ring, the 8 cells around it.
- **Growth:** tile by tile, paid in fragments. Each new tile must be orthogonally next to existing floor.
- **Mining:**
  - A blocking field cell must be mined before it can be floored.
  - Mining turns it into walkable ground, saved as a field tile override.
  - A mined cell can drop fragments.
- **Sieges** spawn from cells just outside the base's floored edge.
- **Breach:**
  - The field map and the base persist.
  - Wild entities spawn at the new zone's tier.
  - Dungeons the player has not entered migrate to the new tier.

## Alternatives considered

- **Keep base space and add a field fixture.** Rejected: it keeps all of today's two-space cost and adds more.
- **Carry the base across breaches by offset and restamp.** This is the pre-0.13.0 slab approach. Rejected: it brings back the slab geometry code that v0.13.0 deleted.
- **Re-found the base in every zone.** Rejected: it throws away the player's build each breach.

## Consequences

- **Unlocks:**
  - A base that grows out of the terrain the player walks.
  - Mining that matters on the field.
  - A single space for rendering and placement.
  - A field that persists, so terrain changes stay put.
- **Breaks:**
  - The save format, which needs a bump of `save::SAVE_FORMAT_VERSION` and a breaking minor release.
  - Every `dev-saves/` template, each of which needs recapturing.
  - The seams in `.claude/rules/seams-base.md` and `seams-sieges.md`. Both need rewriting along with their `seam:` graph entries and the seams skill references.
- **Retires:**
  - The anchor pin trap.
  - `BASE_EXIT_CELL`.
  - `Locale::Base`.
  - The split between base and surface checks.

## Evidence

- `Game::enter_base`, `Game::leave_base`, `lay_starting_pocket`, `BASE_EXIT_CELL` — `crates/engine/src/game/base_space.rs`
- `Game::place_structure`, `require_base`, `PlacementRefusal`, `Game::place_wall_box` — `crates/engine/src/game/base/building.rs`
- `Biome::walkable`, `WorldMap::set_override` — `crates/engine/src/world.rs`
- `move_anchor_to` and its settlement caller — `crates/engine/src/game/settlement_footprint.rs`
- `.claude/rules/seams-base.md`, `.claude/rules/seams-sieges.md`
