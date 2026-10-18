# 0008 Shared derivations are functions of `&World`

Status: Proposed (2026-10-16)

## Context

`tick_inner` (`crates/engine/src/game/turn.rs:241-496`) runs about 49 hand-ordered `Game` passes around one `schedule.run` (`:364`) of about fifteen bevy systems.
Several passes are `&mut Game` methods rather than systems because they need a `Game`-level derivation — `creature_label` for the repair-bay line, `extraction_yield` for the teardown rig — and a system has queries and resources, not a `Game`.
The memory graph records those as forced, and they are, as long as the derivation is a method.

But `Game` is `{ world, schedule }` and nothing else, so every `&self` derivation is already a function of the `World`.
The pattern exists: `power::ledger(&World)` (`game/base/power.rs:155`) is called by `power_grid_system` (`systems.rs:995`) and by `Game` methods, and neither copies the formula.

## Decision

Extract shared derivations as `pub(crate) fn name(&World, …)` beside their domain, with the `Game` method becoming a one-line call.
Then, one pass at a time and only where a pass has no other reason to be hand-ordered, move it into the schedule as an ordered system set.
A pass whose doc comment gives a reason other than "needs `Game`" stays where it is.

## Consequences

The "forced `&mut Game` pass" argument stays true for every pass not yet converted, so no seam breaks.
Systems get explicit ordering, which is what ADR 0001's executor change wants.
The hand-ordered list shrinks, and a new tick pass has a default home.
This follows ADR 0005: a derivation is only extractable once the method it comes from is `&self`.

## Evidence

- `crates/engine/src/game/turn.rs:241-496` — `tick_inner`.
- `crates/engine/src/game/base/power.rs:155` — `ledger(&World)`.
- `crates/engine/src/systems.rs:995` — the system that calls it.
