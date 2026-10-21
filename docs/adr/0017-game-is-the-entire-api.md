# 0017 `Game` is the entire API

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: in `CLAUDE.md` by 2026-09-08; recorded here 2026-10-21
- Pattern: Facade

## Context

The engine is a standalone `bevy_ecs` world.
`crates/gui` depends on `bevy`, which carries the same `bevy_ecs` version, so any handle to the `World` would be usable from the renderer immediately, with nothing in the build to give pause.

## Decision

`Game` (`crates/engine/src/lib.rs`) is the entire API the renderer talks to, through app-core.
`Game.world` is private and has no accessor; never add one.
The renderer reads views and calls methods; it never names a component or runs a query.
The gui frontend is one bevy system, `frame`, holding the app state machine in a resource, rather than idiomatic bevy components, for the same reason.

## Alternatives considered

- A `world()` or `world_mut()` accessor for convenience. Usable at once from gui; the rule would then be held by review alone.
- Sub-facades (`game.base()`, `game.battle()`). Rejected in [0011](0011-restructurings-considered-and-rejected.md); renames without removing coupling.
- Model the frontend as bevy components. Either duplicates game state into a second world or reaches past `Game`.

## Consequences

The engine can change its components freely; only `Game` methods and view types are a contract.
`Game` is large (hundreds of methods), split by topic under `crates/engine/src/game/`.
Engine tests stay inside the crate (`src/tests/`), because an integration test binary would need the forbidden accessor ([0011](0011-restructurings-considered-and-rejected.md)).

## Evidence

- `crates/engine/src/lib.rs` — `Game { world, schedule }`.
- `crates/gui/src/lib.rs` — module doc on the one `frame` system.
- `CLAUDE.md` — Architectural rule.
