# 0008 Shared derivations are functions of `&World`

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-16 (narrowed 2026-10-21)
- Pattern: Shared pure function
- Depends on: [0005](0005-engine-queries-take-self-not-mut-self.md)

## Context

A derivation such as `Game::creature_label` or an extraction yield is needed both by `Game` methods and by code that has only a `World` — a bevy system has queries and resources, not a `Game`.
When the derivation is a method, the second consumer either becomes a `&mut Game` pass or copies the formula, and a copy drifts.
`CLAUDE.md` already says a doc comment claiming to mirror other code must be a call, not a copy.

`Game` is `{ world, schedule }`, so every `&self` derivation is already a function of the `World`.
The pattern exists: `power::ledger(&World, …)` is called by `power_grid_system` and by `Game` methods, and neither copies the formula.

## Decision

Extract a shared derivation as `pub(crate) fn name(&World, …)` beside its domain, with the `Game` method becoming a one-line call.
Every consumer — method, system, view — calls it; none restates it.
Do it when a second consumer appears or a copy is found, not as a sweep.

## Alternatives considered

- Keep derivations as methods and pass `&Game` around. A system cannot hold one.
- Move the hand-ordered `tick_inner` passes into the ECS schedule once their derivations are free functions. Rejected; see [0011](0011-restructurings-considered-and-rejected.md).

## Consequences

The "mirrors must be a call" rule has one shape to point to.
A `&mut Game` pass that exists only to reach a derivation can call the function instead, but stays a `Game` pass ([0021](0021-plain-rust-over-ecs-for-discrete-logic.md)).
Extraction needs the source method to be `&self` first, hence the dependency on 0005.

## Evidence

- `crates/engine/src/game/base/power.rs` — `power::ledger`.
- `crates/engine/src/systems.rs` — `power_grid_system`.
- `crates/engine/src/battle.rs` — `attackers_in_group`, `slot_aggro_weight`, `expected_damage`.
