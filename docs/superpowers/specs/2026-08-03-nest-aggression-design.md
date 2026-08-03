# Nest aggression and the nest cache

Date: 2026-08-03
Status: implemented (2026-08-03) — two decisions moved during
implementation; see the notes marked below.

## The problem

A nest is currently a punching bag. Walking into it deals one hit of the
player's `effective_atk` against no defense; it never retaliates, and
destroying it pays nothing. The only thing you gain is that its guardians
stop respawning. The manual calls a nest "a deliberate risk/reward pocket",
but as built it is neither: the risk is the ordinary wild programs already
standing around it, and the reward is the absence of a nuisance.

Two changes fix that together, and neither works alone:

- **Attacking a nest provokes it.** Its guardians converge on the player and
  engage. The risk becomes the nest itself.
- **Destroying a nest pays a cache.** A reason to accept that risk.

## Scope

In:

- A `Pursuing` marker on nest guardians, set by attacking their nest.
- A per-tick pursuit step that paths pursuers toward the player and starts a
  battle on contact.
- A loot cache granted when a nest's `Durability` reaches 0.
- Persisting nests, the guardian tether and live aggro across save/load.
- The `pathfinding` crate as an engine dependency.

Out:

- Aggro for any hostile that is not a nest guardian. No sight radius, no
  general pursuit. The overworld away from a nest plays exactly as it does
  now; `maybe_ambush` remains the only unprovoked fight.
- Ranged attacks, or any way to damage a nest other than walking into it.
- Nest aggression underground. The Stack has its own guardian model
  (`lair_cleared`) and no `Nest` entity exists down there.

## Existing behaviour this builds on

Verified against the source on 2026-08-03; re-verify before relying on any
of it.

- A nest is an entity with `Nest { species, pending_respawns }`, `Position`,
  `Glyph`, and `Durability { hp: 60, max_hp: 60 }`. It has **no `Structure`
  component**, which is why it is absent from the save (below).
- `Game::attack_nest` (`game/zone.rs`) is reached from `move_player`, which
  checks `find_nest_at` before the ordinary blocking-structure check.
- `Game::despawn_nest` strips `NestGuardian` from every tethered creature
  before despawning, so no guardian is left pointing at a dead entity. It is
  the single door out, called by `attack_nest` and by `stamp_platform`.
- `wander_ai_system` (`systems.rs`) is the only creature AI in the game: a
  random ±1 step on a 2–6 tick cooldown, refused if it would take a
  `NestGuardian` more than `NEST_TETHER_RADIUS` (5, Chebyshev) from its nest.
- `Game::nest_respawn_tick` (`game/upkeep.rs`) runs from `tick_inner` and
  spends `Nest::pending_respawns`, which `finish_member` and the tame path
  push onto when a guardian leaves the world.
- `Game::gather_pack` (`game/combat.rs`) collects every `Hostile` within
  `swarm_radius` of an anchor and truncates to the danger curve's ceiling.
- `Game::award_loot` (`game/combat_rewards.rs`) is the model for the cache:
  a `work_resource` quantity roll, an equipment-drop table roll per entry,
  and — for a boss — a `BOSS_PORTAL_FRAGMENT_DROP` cache of craft currency.

## Design

### Aggro is a marker, not a state machine

New component:

```rust
/// Set on a `NestGuardian` whose nest has been attacked: it abandons its
/// tether and closes on the player until it makes contact, is killed, or
/// leashes off. No target field — the player is the only thing anything
/// in this game pursues.
pub struct Pursuing;
```

There is deliberately no `Aggro { target }`, no timer field and no
`Idle | Chasing | Returning` enum. The chase ends spatially (the leash) or
by the nest dying, and "returning" is just ordinary wandering once the
tether bug below is fixed. A second thing that can be pursued, or a chase
that ends on a clock, is the signal to revisit this — not a reason to build
either now.

### The tether bug this feature would otherwise introduce

`wander_ai_system` refuses a move whose *new* Chebyshev distance from the
nest exceeds `NEST_TETHER_RADIUS`. Today nothing can put a guardian outside
that radius, so the check is total. A pursuer that chases the player 15
tiles and then leashes off has **no legal move at all** — every neighbour is
still beyond 5 — and stands frozen for the rest of the run.

Fix at the check: refuse only when the new distance exceeds the radius
**and** is no closer than the current distance. A displaced guardian then
walks home on its own, which is why no `Returning` state is needed. This is
a one-line change with its own reproducer test, and it must land before or
with the pursuit step, never after.

### Pathing: one distance field per tick

Add `pathfinding = "4"` to `crates/engine/Cargo.toml`.

Once per tick, when any pursuer exists, run `pathfinding::dijkstra_all` from
the **player's** tile outward over walkable neighbours, producing a cost
field. Each pursuer then steps to whichever of its eight neighbours has the
lowest cost in that field.

One search regardless of swarm size, and pursuers converge from different
sides for free. Per-pursuer `astar` was considered and rejected: same cost
at these sizes but N searches where this needs one, and each pursuer blind
to the others. It becomes the right call only if aggro ever generalises to
scattered individuals chasing different targets, which this scope rules out.

**The search must be bounded.** The map is infinite and generates chunks
lazily on `WorldMap::tile`, so an unbounded search that finds no route would
generate chunks outward until it exhausted memory. The successor function
rejects any tile outside a box of `NEST_AGGRO_LEASH_RADIUS + margin` around
the **player** — not the nest, corrected below — which caps both the search
and the chunks it forces into existence.

> **Implementation note:** shipped centred on the player, not the nest as
> the box description above once implied. One box around the player is the
> same bound with a simpler centre, and it holds however many nests are
> provoked in the same tick — a per-nest box would have needed one search
> per nest instead of one for the whole swarm. See `Game::nest_aggro_tick`
> (`game/turn.rs`).

Two tiles are never successors:

- Anything not `walkable`.
- Anything with `Biome::Platform`. The base slab stays the one safe ground,
  as `maybe_ambush` and `stamp_platform` already establish. A leash measured
  from the nest cannot guarantee this on its own, since a nest can stand
  within leash range of the base.

A pursuer whose own tile is absent from the field has no route to the player
— it stands inside an enclosure, or the player is outside the box. It skips
its step and wanders. This is a legitimate outcome, not an error.

> **Implementation note:** shipped removing `Pursuing` outright rather than
> "skips its step and wanders" as written above. A pursuer absent from the
> field is exactly the guardian `wander_ai_system` would otherwise also
> skip (it excludes anything `Pursuing`), so "wanders" would have left it
> frozen solid forever — while still paying for a full field build on its
> behalf every tick from then on. Dropping the marker is the same give-up
> the leash check already performs, just triggered by unreachability
> instead of distance.
>
> This has a consequence worth recording in its own right: standing
> anywhere in the base slab's interior makes every one of the player's
> eight neighbours `Biome::Platform`, so the field the pursuit step builds
> comes back holding only the player's own tile — the same shape
> `pursuit_field`'s enclosed-origin case produces. Every pursuer currently
> `Pursuing`, however far off its own chase actually is, reads as absent
> from that field, so reaching home disbands the *whole* swarm at once,
> not just whichever guardian happened to be closest. **This is the
> intended rule, not a bug: the base is where a chase ends, zone-wide.**
> See `Game::nest_aggro_tick` (`game/turn.rs`) and
> `standing_inside_the_base_slab_clears_every_pursuer_zone_wide`
> (`tests/zone.rs`).

### One tick of pursuit

New `Game::nest_aggro_tick`, called from `tick_inner` immediately after
`nest_respawn_tick`. It is a `Game` method rather than a bevy system for the
same reason `spawn_nest_guardian` is: it can call `start_battle`, which a
system cannot reach.

```
if game over or a battle is active: return
clear Pursuing on any pursuer beyond the leash radius from its nest
collect pursuers; if none: return
build the bounded cost field from the player's tile
for step in 0..NEST_PURSUIT_STEPS_PER_TICK:
    for each pursuer:
        move it one tile downhill, if a downhill neighbour exists
        if it is now Chebyshev-adjacent to the player:
            start_battle(gather_pack(pursuer)); return
```

`gather_pack` already folds in every hostile within the swarm radius, so
whoever arrived fights and whoever did not keeps coming. The step loop
returns the moment a battle starts, so nothing moves during a fight.

`NEST_PURSUIT_STEPS_PER_TICK` is the tunable speed knob: 1 is player speed —
you can outrun a swarm in a straight line but never shake it, and it catches
you the moment you stop to work, rest, or swing at the nest again. Above 1
they will reach you and the only question is where you would rather fight.

Ordering matters and is load-bearing. `move_player` and `attack_nest` both
call `tick` after acting, so the sequence is: player acts → pursuers step →
contact. `attack_nest` sets `Pursuing` **before** its `tick`, so the swarm
moves on the same tick as the hit that provoked it.

### What sets and clears `Pursuing`

Set:

- `attack_nest`, on every living guardian of the nest that was hit, before
  its tick. Every hit re-applies it, so a guardian that leashed off is
  provoked again by the next swing.
- `nest_respawn_tick`, on a fresh guardian whose nest already has at least
  one pursuer. A besieged nest keeps feeding the chase — the pressure clock.

Cleared:

- Beyond `NEST_AGGRO_LEASH_RADIUS` (15, Chebyshev, measured from the nest,
  not from where the chase started). Checked at the top of the pursuit step,
  before the field is built, so a leashed pursuer costs nothing.
- `despawn_nest`, alongside the `NestGuardian` it already strips. One place,
  covering both callers.
- The tame path in `combat_rewards.rs`, which already removes
  `(Hostile, WanderAi, NestGuardian)` from a decompiled front-liner.
  `Pursuing` joins that tuple.
- A successful `battle_flee`, on every entity that was actually in that
  battle. See the Implementation note below — this reverses what this
  spec originally said.

> **Implementation note:** this spec originally said "a guardian the
> player fled from stays `Pursuing`: running away is not a reason for a
> swarm to calm down. This needs no rule of its own — nothing in the flee
> path touches the marker." Both halves of that turned out to be false,
> for the same reason the field-absence deviation above exists: `tick`
> runs immediately after `end_battle`, which moves no one and — as
> originally specified — cleared nothing, so `nest_aggro_tick` found the
> same pack still exactly as adjacent as it had been mid-fight and
> re-engaged before the player's next input ever arrived. A jack-out
> against a nest guardian was mechanically impossible: every attempt paid
> `apply_setback_xp_penalty` for nothing, and under permadeath that is a
> death sentence, not friction.
>
> A movement-based fix was tried first and rejected: stepping the player
> one tile away before the tick, on the reasoning that adjacency alone
> would be broken. It wasn't — `NEST_PURSUIT_STEPS_PER_TICK` exactly
> matches that one-tile distance, so the guardian's own ordinary step
> (which fires inside that same tick, immediately after the move) closes
> the gap straight back to adjacency regardless of which of the player's
> eight neighbours was picked, in any open terrain. The math doesn't
> depend on the pursuer being unusually fast — it's exactly as fast as
> the escape, which is what the tuning already promises ("you outrun a
> swarm in a straight line but never shake it").
>
> Clearing `Pursuing` on the battle's own members — collected before
> `end_battle` drops `BattleState`, so a pursuer that was never gathered
> into this fight keeps chasing — sidesteps that arithmetic instead of
> fighting it: escape is guaranteed, not merely probable. `NestGuardian`
> survives untouched, so a shaken guardian resumes ordinary tethered
> wandering exactly like a `despawn_nest` survivor, and the nest
> re-provokes it the next time `attack_nest` lands a hit. A failed
> jack-out attempt shakes nobody. See `Game::battle_flee`
> (`game/combat_teardown.rs`).

### The cache

Granted by `attack_nest` on the hit that brings `Durability` to 0, before
`despawn_nest`, since it reads the nest's species.

Contents derive from the nest's **species data**, not from a new schema
field:

- `NEST_CACHE_WORK_RESOURCE_MULT` × a `WORK_RESOURCE_DROP` roll of the
  species' `work_resource`.
- A `NEST_CACHE_FRAGMENTS` roll of craft currency, scaled by zone (pinned
  below — this was left open here and decided during implementation), in
  the same shape as the boss cache in `award_loot` grants a flat range.
- `NEST_CACHE_EQUIPMENT_ROLLS` passes over the species' equipment drop table
  (`equipment_drops_for`), each entry rolled at its own chance.

> **Implementation note:** "scaled by zone" above was deliberately left
> unspecified. It shipped as `NEST_CACHE_FRAGMENT_ZONE_BONUS` added to the
> `NEST_CACHE_FRAGMENTS` roll once per zone below the nest's own zone — a
> deeper nest, whose guardians already scale with zone and distance, stays
> worth clearing rather than being worth a flat amount everywhere. This is
> a separate mechanism from `BOSS_PORTAL_FRAGMENT_DROP`, which does not
> scale by zone at all; "mirrors the boss cache" above refers only to the
> shape (a currency grant alongside the resource and equipment rolls), not
> the formula. See `Game::grant_nest_cache` (`game/zone.rs`).

Nothing is added to `SpeciesDef`, so a modded nesting species gets a
sensible cache for free — content stays data, and the magnitudes live in
`tuning.rs` where difficulty belongs.

Logged as `MessageKind::Loot` so the lines survive
`retain_outcomes_since_battle` and follow the player onto the map. A
non-lethal hit pays nothing; a half-destroyed nest is worth nothing yet,
which is what makes the last swing matter.

No XP: the guardians are the XP.

### New tuning constants

All in `crates/engine/src/tuning.rs`, in the existing nests section.

| Constant | Proposed | Meaning |
|---|---|---|
| `NEST_PURSUIT_STEPS_PER_TICK` | `1` | Tiles a pursuer covers per tick. 1 = player speed. |
| `NEST_AGGRO_LEASH_RADIUS` | `15` | Chebyshev distance from the **nest** past which a pursuer gives up. |
| `NEST_PATH_SEARCH_MARGIN` | `5` | Added to the leash radius to bound the cost-field box. |
| `NEST_CACHE_WORK_RESOURCE_MULT` | `4` | Multiplier on the ordinary work-resource drop roll. |
| `NEST_CACHE_FRAGMENTS` | `2..=5` | Craft currency from a destroyed nest, before zone scaling. |
| `NEST_CACHE_FRAGMENT_ZONE_BONUS` | `1` | Added to the fragments roll per zone below the nest's own — the zone scaling left open above. |
| `NEST_CACHE_EQUIPMENT_ROLLS` | `3` | Passes over the species' equipment drop table. |

Every one of these is arithmetic-plausible and unplayed. `balance_sim.rs`
gates none of them — it models no map, no pursuit and no nests. They are
retune-on-play knobs, and the spec says so rather than implying the numbers
are settled.

### Persistence

`SAVE_FORMAT_VERSION` 18 → 19.

New in `save.rs`:

```rust
pub struct NestSave {
    pub species: SpeciesId,
    pub position: (i32, i32),
    pub durability: u32,
    pub pending_respawns: Vec<u32>,
}
```

`SaveData` gains `pub nests: Vec<NestSave>`. `CreatureSave` gains:

- `pub nest_position: Option<(i32, i32)>` — the tether, keyed by tile rather
  than entity because entity ids do not survive a round trip. This follows
  `CronjobSave::target_position`, the existing precedent for exactly this
  problem. One nest per tile, so the key is unambiguous.
- `pub pursuing: bool`.

Load order matters: nests must be spawned before creatures, so a creature's
`nest_position` can resolve to a live entity. A `nest_position` that matches
no saved nest drops the tether and the creature loads as an ordinary wild
program — cheaper than failing the load over it.

This is what makes the reward safe. Without it, save/reload deletes every
nest in the zone, which both frees the player from any swarm and destroys a
nest they were 90% through, cache and all. A reward that a reload can
launder is a reward that teaches the player to reload.

## Testing

Engine unit tests. New fixtures go in `crates/engine/src/tests/support.rs`;
look there before writing one.

Aggro:

- Attacking a nest sets `Pursuing` on every living guardian of that nest,
  and on no guardian of a different nest.
- A pursuer closes distance on the player around a concave unwalkable
  region, where a greedy step-toward-the-player would stall. This is the
  test that justifies the dependency; without it, `pathfinding` is not
  earning its place.
- A pursuer that ends its step adjacent to the player starts a battle, and
  that battle contains more than just the pursuer that made contact.
- A pursuer beyond `NEST_AGGRO_LEASH_RADIUS` from its nest loses
  `Pursuing`.
- Pursuers never step onto a `Biome::Platform` tile.
- Destroying a nest clears `Pursuing` along with `NestGuardian`.
- A guardian respawned at a nest that already has pursuers is itself
  `Pursuing`; one respawned at a calm nest is not.
- `nest_aggro_tick` is a no-op while a battle is active.

Tether:

- A guardian placed outside its tether radius walks back toward its nest
  rather than freezing. This is the reproducer for the bug in §"The tether
  bug"; write it first and watch it fail.

Cache:

- The hit that brings a nest to 0 grants its species' work resource, craft
  currency and equipment rolls; a non-lethal hit grants nothing.
- The cache lines are `MessageKind::Loot`.

Persistence:

- A save/load round trip preserves a nest's position, durability and
  `pending_respawns`; its guardians' tethers; and live `Pursuing`.
- A `CreatureSave` whose `nest_position` matches no nest loads as an
  ordinary wild program.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`cargo test -p feral-processes-engine balance_sim` is expected to be
unaffected — if a curve moves, something touched a shared constant by
accident.

## Documentation obligations

- `docs/manual.md` §Nests — rewrite. It currently states a nest "never
  retaliates" and describes no reward; both become false.
- `CHANGELOG.md` — the save-format bump above all.
- Root `README.md` — check for any claim about nests or the overworld being
  passive.
- No `assets/*/README.md` change: no schema field is added.

## Open questions, deliberately deferred

- **Retuning after play.** The six constants above are unplayed. The two
  most likely to be wrong are `NEST_PURSUIT_STEPS_PER_TICK` (1 may make a
  swarm trivially kiteable) and the besieged-nest respawn stream, which as
  designed means a slow player never runs out of enemies.
- **Nests near the base.** The leash is measured from the nest, so a nest
  within 15 tiles of the base can put pursuers on the doorstep. Pursuers
  cannot step onto the platform, so they will mill at its edge. Whether that
  reads as a siege or as a bug is a play question.
