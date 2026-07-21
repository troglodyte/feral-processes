# Wild population cap: raise it, cull distant creatures instead

## Problem

Earlier today we fixed two bugs in `Game::maybe_spawn_wild_creature`'s
population cap (`crates/engine/src/lib.rs`, commit `e21b1d0`):

1. The cap counted every `Creature` entity, including tamed party
   members/cronjob workers, not just wild ones — fixed by scoping the
   count to `Hostile` only.
2. The cap was global across the whole map, so a wild population the
   player had wandered away from (which never despawns on its own) could
   permanently block new spawns near the player's current position —
   fixed by scoping the count to `Hostile` creatures within
   `WILD_POPULATION_CAP_RADIUS` (40 tiles) of the player.

Both fixes are shipped and tested. This spec is a follow-up: we have CPU
and memory headroom to support a much larger total wild population (100
instead of 24), but simply raising the radius-scoped cap's threshold
doesn't address the underlying reason wild creatures pile up in the first
place — they never despawn as the player leaves an area, so total
world-wide entity count (and the AI/tick cost of simulating all of them)
grows unbounded over a long play session even though most of them are far
from the player and will likely never be encountered again.

## Decision

Replace the radius-scoped cap from commit `e21b1d0` with a single
**global** cap (`WILD_CREATURE_CAP`, raised from 24 to 100), paired with
active culling: when the cap is reached and a new wild creature wants to
spawn, despawn the `Hostile` creature farthest from the player's current
position first, freeing a slot. This directly bounds total simulated wild
population (the performance goal) while guaranteeing spawns near the
player are never blocked by a population that accumulated somewhere else
(the correctness goal from today's second fix) — one mechanism serves
both purposes, so the `WILD_POPULATION_CAP_RADIUS` scoping this replaces
can be removed rather than kept alongside it.

**Despawn eligibility:** every `Hostile` creature counts, including
`NestGuardian`s — per earlier discussion, a nest's guardian count becomes
best-effort rather than exact once this ships. A culled guardian is
despawned directly (not through the normal death-handling path), so it
does **not** push onto its `Nest`'s `pending_respawns` — the nest simply
ends up with one fewer guardian until the player revisits and the normal
kill/tame/respawn cycle rebuilds it. This is a deliberate simplification:
teaching the culling path to distinguish "guardian defeated" from
"guardian culled for space" and thread that into `Nest::pending_respawns`
correctly would meaningfully complicate the change for a rare, low-stakes
edge case (nests are already tethered near where they spawned, so they're
one of the least likely candidates to actually be the "farthest" creature
picked for culling).

## Design

- Rename/repurpose `WILD_POPULATION_CAP_RADIUS` and the local `24`
  literal in `maybe_spawn_wild_creature`
  (`crates/engine/src/lib.rs:3079`) into one constant:
  `const WILD_CREATURE_CAP: u32 = 100;` — no radius constant needed
  anymore.
- `maybe_spawn_wild_creature` changes from "count nearby Hostile
  creatures, bail if at cap" to:
  1. Roll the existing 5% spawn chance first (unchanged) — culling is
     wasted work if nothing was going to spawn anyway.
  2. If the roll succeeds, check the *global* `Hostile` count. If it's
     `>= WILD_CREATURE_CAP`, despawn the single `Hostile` entity with the
     greatest Chebyshev distance from the player's current position
     (same distance metric `distance_stat_multiplier` already uses).
  3. Proceed with the existing `try_spawn_habitat_creature` call as
     today.
- A pack spawn (`try_spawn_habitat_creature`'s `group_size`, up to
  `zone + 1` members) can still push the total a few over
  `WILD_CREATURE_CAP` in one roll — that's fine, the next roll's cull
  brings it back down. Don't loop the cull to pre-clear room for an
  entire prospective pack; that couples this function to pack-size logic
  it doesn't otherwise know about.
- No change to `try_spawn_habitat_creature`, `spawn_wild_creature`,
  `spawn_nest`, `spawn_nest_guardian`, or any nest respawn/bookkeeping
  logic — the cull is a plain `World::despawn` on an existing entity, the
  same primitive `enter_next_zone` already uses for stale entities.

## Changes

1. `crates/engine/src/lib.rs`:
   - Remove `WILD_POPULATION_CAP_RADIUS` (const, ~line 92-97 after
     today's edits) and add `WILD_CREATURE_CAP: u32 = 100` in its place,
     with an updated doc comment describing the cull-based design above
     instead of the radius scoping it replaces.
   - Rewrite `maybe_spawn_wild_creature` (~line 3079) per the Design
     section: roll first, then cap-check + cull globally, then spawn.
   - Update/replace the two existing regression tests
     (`wild_spawn_cap_is_not_exhausted_by_tamed_creatures`,
     `wild_spawn_cap_is_not_exhausted_by_creatures_far_from_the_player`)
     — both still describe correct end-state behavior (spawns aren't
     blocked by tamed creatures or by a distant population) and should
     keep passing conceptually, but the second one specifically
     exercises the radius-scoping mechanism being removed, so it needs
     rewriting to assert the new behavior: a large distant population
     gets culled (shrinks) rather than simply not counted, while a new
     creature still successfully spawns near the player.
   - Add a new test asserting cull eligibility: with the cap already
     reached by `NestGuardian` entities scattered far from the player,
     confirm a spawn near the player still succeeds and the global
     `Hostile` count doesn't just grow past the cap (i.e., the farthest
     one — a guardian — does get culled), without asserting anything
     about that nest's `pending_respawns` state (best-effort, not
     tested).
2. No changes needed to `assets/structures/`, `assets/species/`, or any
   `README.md` — this is an internal balancing mechanism, not
   player-facing content or a documented mechanic. (Worth a one-line
   changelog entry per project convention if the actual gameplay
   difference — more wild creatures visible at once, ones near you never
   despawn — is judged noticeable enough; decide at implementation time.)

## Out of scope

- No UI/frontend change — culled creatures simply stop appearing next
  time the player looks; there's no message logged for a cull (compare:
  `enter_next_zone`'s stale-entity despawn is also silent).
- No change to `Tamed` creatures, structures, or any other entity kind —
  culling only ever targets `Hostile` entities.
- No attempt to make nest guardian culling respawn-aware (see Decision).
