# 0022 RNG never depends on query order

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-10-19 (077f90ae); recorded here 2026-10-21
- Pattern: Deterministic ordering

## Context

Seeded tests and `balance_sim` assume the RNG stream is a function of the seed.
bevy's query iteration order is not stable: adding an unrelated `Resource` or component shifts it.
A loop that draws from `GameRng` while walking a query then shifts every later draw, and tests that were passing on seed luck start failing far from the change.

## Decision

Inside a loop over a query, sort by a stable id before drawing.
Stable ids are values the game mints (`components::ProgramId`, tile positions, def ids), not `Entity` allocation order.
State that is walked in order lives in an ordered container (`Vec`, `BTreeMap`), never a `HashMap`.

## Alternatives considered

- Accept the shifts and re-seed tests when they move. Hides real regressions among seed noise.
- Draw once per tick and derive per-entity values by hashing. Workable for some cases, but changes every existing stream at once.

## Consequences

A new resource or component cannot move an unrelated test's numbers.
New resources that would shift order are sometimes kept as fields of an existing one instead, as `TacticalBattle::rewards` is.

## Evidence

- `crates/engine/src/tactical/mod.rs` — `TacticalBattle`'s `bodies` `Vec` and `rewards` field docs.
- `crates/engine/src/components/basics.rs` — `ProgramId`.
- Memory: `new-resource-shifts-query-iteration-order`, `rng-stream-shift-exposes-seed-luck-tests`.
