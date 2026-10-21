# 0020 Functional core, imperative shell for `impl Game`

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-10-20 (99636e2a); recorded here 2026-10-21
- Pattern: Functional core, imperative shell

## Context

A `Game` method that reads the `World`, rolls dice and writes results in one body can only be tested by building a whole `Game`, and its RNG draws are tangled with its queries.
A formula restated in a second place to avoid that drifts from the first.

## Decision

An `impl Game` method has three steps.
Gather: `&self` builds a plain input struct from the `World`, sorted by stable id.
Decide: a pure `fn(input, &mut Rng) -> Outcome` that takes no `&World` or `&Game`.
Apply: `&mut self` writes the `Outcome` back and notifies.
The decide step is tested directly.
`Outcome` is concrete per feature, never one generic `Effect` enum.

A doc comment claiming to mirror other code must be a call, not a copy: extract a pure function both sides call.

Extract when already changing a `game/*.rs` method, not as a migration.
Keep the RNG draw order unchanged and check `balance_sim` afterwards.

## Alternatives considered

- A generic `Effect` enum applied by one interpreter. One type grows every variant of every feature.
- Test through `Game` only. Slow, and background systems interfere with assertions.

## Consequences

Decide steps are cheap unit tests.
Existing methods move over one at a time, so both shapes coexist.

## Evidence

- `crates/engine/src/game/spawning.rs` — `roll_summon_tier`.
- `crates/engine/src/game/combat_rewards.rs` — `pick_affix`.
- `crates/engine/src/battle.rs` — `attackers_in_group`, `slot_aggro_weight`, `expected_damage`.
- `crates/engine/src/systems.rs` — `node_payout`.
