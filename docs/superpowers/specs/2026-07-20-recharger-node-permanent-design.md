# Recharger Node: permanent structure

## Problem

The Recharger Node (`assets/structures/recharger_node.ron`) currently sets
`temporary: Some((max_ticks: 20))`, so it collapses 20 ordinary game-clock
ticks after being deployed, with no refund. This makes it a disposable
convenience rather than a real piece of base infrastructure.

## Decision

Make the Recharger Node a permanent structure by removing its `temporary`
field. No new placement restriction is needed: `Game::place_structure`
already requires every non-Home structure (Recharger Node included) to be
deployed within `MAX_BUILD_DISTANCE_FROM_HOME` (15 tiles) of the Home
structure. That existing rule is what "only near the base" means here —
there is nothing base-specific left to add.

The generic `temporary` structure field and its `Temporary` component stay
in the engine and schema, available to other or modded structures — this
change only stops using it on the Recharger Node.

## Changes

1. `assets/structures/recharger_node.ron` — delete the `temporary: Some((max_ticks: 20))`
   line. Omitting the field defaults to permanent (existing schema
   behavior, already documented in `assets/structures/README.md`). The
   README's `temporary` field doc comment cites the Recharger Node by name
   as its example ("a structure that also sets `enables_rest` (like the
   Recharger Node) isn't worn down any faster..."); since it stops setting
   `temporary`, that name-check needs to be dropped from the example.
2. `crates/engine/src/lib.rs` test updates:
   - `spawn_recharger_node_at_player` test helper: stop reading
     `temporary.max_ticks` from the schema and stop attaching a `Temporary`
     component to the spawned test entity.
   - `recharger_node_structure_loads_with_the_expected_rest_and_temporary_schema`:
     rename (drop "temporary" from the name) and assert
     `def.temporary.is_none()` instead of checking `max_ticks`.
   - Delete `recharger_node_collapses_after_its_lifespan_of_ordinary_ticks`
     and `resting_does_not_age_the_recharger_node` — both exercise decay
     behavior that no longer exists.

## Out of scope

- No change to `place_structure`, `MAX_BUILD_DISTANCE_FROM_HOME`,
  `enables_rest`, or the `Temporary`/`temporary` mechanism itself.
- No new radius or base-proximity concept — the existing 15-tile Home rule
  is sufficient per user confirmation.
