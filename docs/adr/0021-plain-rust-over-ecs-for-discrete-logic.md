# 0021 Plain Rust over ECS for discrete logic

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-10-19 (077f90ae); recorded here 2026-10-21
- Pattern: —

## Context

The engine is a `bevy_ecs` world, so the path of least resistance for any new state is a component reached by `world.get`.
For turn-based logic — one fight, one board, one contract — that scatters state that has one owner across entities, makes it reachable from any system, and ties its behaviour to query iteration order.

## Decision

New discrete or turn-based logic is plain Rust: structs owned in one place, plus pure functions over them.
ECS is for many entities ticking (staff, wanderers, tasks).
Existing ECS code is not migrated for its own sake.

## Alternatives considered

- Everything in ECS, for uniformity. Puts owned, ordered state behind unordered queries.
- Migrate existing ECS logic to plain structs. Churn with no bug to fix.
- Move hand-ordered `tick_inner` passes into the schedule. Rejected in [0011](0011-restructurings-considered-and-rejected.md) for this reason.

## Consequences

A tactical fight's board and bodies live in one `TacticalBattle` resource with a stable-ordered `Vec`, not on entity components; its coordinates never touch `Position`.
Some passes stay `Game` methods on purpose, which seam rules record (`Game::run_repair_bays`).

## Evidence

- `crates/engine/src/tactical/mod.rs` — `TacticalBattle`.
- `crates/engine/src/game/base/repair.rs` — `run_repair_bays`.
- `.claude/rules/seams-tactical.md` — "`Position` is never written".
