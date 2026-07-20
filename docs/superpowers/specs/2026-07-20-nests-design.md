# Wild Creature Nests — Design

## Summary

Certain wild-creature species can spawn as a **Nest**: a stationary,
destructible map object that keeps 2-5 guardians of its species alive
around it. Guardians stay tethered within a fixed radius of the nest;
killing or taming one queues a replacement a fixed number of ticks later.
Destroying the nest severs the tether immediately — surviving guardians
revert to ordinary, unrestricted wandering, and respawns stop.

## Goals

- A wild-spawn variant that gives a place a persistent "danger zone" flavor
  rather than every hostile encounter being a one-off.
- Fully moddable: which species can nest is a data flag on `SpeciesDef`,
  not a hardcoded species list.
- Reuses existing ECS patterns (spawn/despawn, `Durability`,
  `WanderAi`, generic `view_entities` rendering) rather than introducing a
  parallel subsystem.

## Non-goals

- No guaranteed loot reward for destroying a nest (unlike a boss kill's
  guaranteed Portal Fragment cache) — the payoff is simply that it stops
  producing guardians. Can be revisited later if it proves unsatisfying.
- No new turn-based battle screen for nest destruction — see the
  "Destruction mechanic" section for why this is deliberately *not* routed
  through `BattleState`.
- Bosses never nest — nest eligibility only applies to ordinary
  (non-boss) species.

## Data model

### `SpeciesDef` schema change

New field on `SpeciesDef` (`crates/engine/src/species.rs`):

```rust
/// Whether this species can spawn as a Nest (see `components::Nest`)
/// instead of an ordinary lone creature/pack during habitat spawning.
/// `#[serde(default)]` so existing `.ron` files (including mods) parse
/// unchanged, defaulting to no nesting.
#[serde(default)]
pub can_nest: bool,
```

`assets/species/README.md` gets a matching entry documenting the field,
per this project's standing rule that the schema docs are updated in the
same change as any `SpeciesDef` field change.

Starting set of species with `can_nest: true`: Scrapper, Worm, Wraith,
Trojan (the existing swarm/pack-flavored Medium-tier species). This is
just an initial content choice, trivially adjustable per-species and by
mods — not an engine constraint.

### New engine-only constants

Uniform across every nesting species (mirrors how `BOSS_SPAWN_CHANCE`
isn't per-species), defined near the existing spawn-tuning constants in
`crates/engine/src/lib.rs`:

- `NEST_SPAWN_CHANCE: f64 = 0.06` — chance, when a nest-eligible species is
  picked by the habitat roll, that a Nest spawns instead of an ordinary
  pack.
- `NEST_TETHER_RADIUS: i32 = 5` — Chebyshev distance a guardian may wander
  from its nest.
- `NEST_GUARDIAN_MIN: u32 = 2`, `NEST_GUARDIAN_MAX: u32 = 5` — initial
  guardian count range.
- `NEST_RESPAWN_TICKS: u32 = 10` — ticks between a guardian's
  death/taming and its replacement spawning.
- `NEST_DURABILITY: u32 = 60` — a nest's starting/max `Durability`.

## Components (`crates/engine/src/components.rs`)

```rust
/// A stationary spawner for a wild species — see the nests design doc.
/// Present on the nest entity itself (which also carries `Position`,
/// `Glyph`, and `Durability`, all reused as-is).
#[derive(Component, Clone, Debug)]
pub struct Nest {
    pub species: SpeciesId,
    /// Ticks remaining until each queued replacement guardian spawns —
    /// one entry per guardian currently missing from the nest's original
    /// count. Emptied naturally once every guardian is back at full
    /// strength.
    pub pending_respawns: Vec<u32>,
}

/// Tags a wild creature as tethered to a `Nest` — see `wander_ai_system`'s
/// radius check. Removed (not the creature) when its nest is destroyed or
/// when the creature is killed/tamed, at which point it either despawns
/// or wanders/behaves like any other creature.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}
```

## Spawning flow

In `Game::try_spawn_habitat_creature` (`crates/engine/src/lib.rs`), after
the existing boss-vs-ordinary pick logic settles on an ordinary species
`pick`:

1. If `species_db.get(&pick).can_nest`, roll `NEST_SPAWN_CHANCE`.
2. On a hit, spawn a `Nest` entity at `(x, y)` (`Position`, `Glyph` using a
   fixed `'N'` glyph in the species' own color so it reads as distinct
   from a live creature at a glance, `Durability { hp: NEST_DURABILITY,
   max_hp: NEST_DURABILITY }`, `Nest { species: pick.clone(),
   pending_respawns: vec![] }`).
3. Immediately spawn a random `NEST_GUARDIAN_MIN..=MAX` count of guardians
   via the existing `spawn_wild_creature` (unchanged — zone/distance stat
   scaling still applies), clustered loosely around the nest the same way
   pack members already cluster around a pack anchor, each additionally
   tagged with `NestGuardian { nest: <nest entity> }`.
4. On a miss (or a non-nesting species), spawning proceeds exactly as
   today.

`spawn_wild_creature`'s signature changes from `fn spawn_wild_creature(...)`
to `fn spawn_wild_creature(...) -> Entity` so callers can tag the result;
existing call sites that ignore the return value are unaffected.

## Tethering

`wander_ai_system` (`crates/engine/src/systems.rs`) gains a second,
read-only query over `(&Position, &Nest)` (via `Entity` lookup) to resolve
a guardian's tether target. For an entity with `NestGuardian`, a candidate
move is only applied if the resulting position stays within
`NEST_TETHER_RADIUS` (Chebyshev) of the nest's position — otherwise the
move is skipped for that tick, same as today's unwalkable-tile case.
Entities without `NestGuardian` are unaffected — identical behavior to
today.

## Destruction — bump attack, not a battle screen

This is the one place the design deliberately diverges from the most
literal reading of "the player fights the nest": `BattleState` /
`BattleView` / `battle_attack` / `battle_decompile` / `battle_flee` /
`wild_retaliate` are all built assuming the target is a `Creature` with a
species (decompile odds, retaliation moves, companion special abilities).
Routing a nest through that machinery would mean special-casing nearly
every method in the battle module to handle a target with no `Creature`,
no `Stats`, and no species — a lot of invasive change for a target that's
explicitly meant to never fight back.

Instead, `Game::move_player` gets a new early check (alongside its
existing wild-creature/zone-portal/blocking-structure checks): walking
into a tile occupied by a `Nest` deals one hit of `effective_atk` (vs. 0
defense — nests have no defense stat, only a Durability pool) to the
nest's `Durability`, logs a strike message, and consumes the move/tick —
mechanically equivalent to "attack it, no retaliation," just without a
redundant parallel UI for a fight that only ever has one side acting.

On reaching 0 `Durability`:

- Log a destruction message.
- Query every `NestGuardian` pointing at this nest entity and remove the
  `NestGuardian` component from each (the creature itself is untouched —
  it keeps its `WanderAi` and simply loses its tether, per "wander like
  normal").
- Despawn the nest entity. Its `pending_respawns` list disappears with
  it, so any already-queued respawns are implicitly cancelled — nothing
  else needs to reference or clean up that list separately.

### Raid-system exclusion

`Game::raid_check`'s target query is currently generic —
`query_filtered::<Entity, With<Durability>>()` — which would otherwise
make a Nest eligible to be picked as a raid target. That's semantically
wrong (a nest is a hostile-owned object, not part of the player's base)
and gets fixed by adding `Without<Nest>` to that query's filter.

`structure_regen`'s query is left generic (`&mut Durability`, no
filter) — a nest passively healing over time like any other
`Durability`-holder is a reasonable, free emergent behavior (an
unattended nest becomes tougher to clear), not something that needs
special-casing out.

## Respawns

A new `nest_respawn_system` (`crates/engine/src/systems.rs`), run once per
game tick alongside the other per-tick systems:

- For every `Nest`, decrement each entry in `pending_respawns` by 1.
- Any entry that reaches 0 is removed, and one new guardian is spawned
  near the nest (same loose clustering as initial spawn, tagged
  `NestGuardian`).

Something has to *push* an entry onto `pending_respawns` in the first
place. Two existing removal paths get a small addition:

- `Game::finish_front_pack_member` (a guardian is killed in battle): before
  despawning the front pack member, check for `NestGuardian`. If present
  and its nest entity still exists (i.e. `world.get::<Nest>(nest)` is
  `Some`), push `NEST_RESPAWN_TICKS` onto that nest's `pending_respawns`.
- `Game::battle_decompile`'s success branch (a guardian is tamed): same
  check and push, immediately before/alongside the existing
  `remove::<(Hostile, WanderAi)>()` call — and `NestGuardian` is removed
  from the now-tamed creature too, for the same reason `Hostile`/`WanderAi`
  are: it's no longer a wild, tethered thing.

If the nest no longer exists at the time of the kill/tame (already
destroyed), nothing is pushed — respawns simply stop, which is exactly
the desired "nest destroyed → no more guardians" behavior without any
extra bookkeeping.

## Rendering / UI

No new UI work needed. `Game::view_entities` already reports any entity
carrying `(Position, Glyph)` generically, and separately reports
`Durability` as an `(hp, max_hp)` pair for anything that has it — a Nest
gets a glyph and a durability bar on both the GUI and TUI for free, the
same way a player structure does today.

`Game::entity_label` currently falls back to `"You"` for any entity that's
neither a `Creature` nor a `Structure` — this needs a new branch for
`Nest`, returning something like `"{species name} Nest"`, so nests don't
mislabel themselves in any UI surface that calls `entity_label`
(`view_entities`, `raid_check`'s log line, etc.).

## Testing

- Unit tests in `crates/engine/src/systems.rs` / `lib.rs` covering:
  - A guardian's move is rejected once it would exceed
    `NEST_TETHER_RADIUS` from its nest.
  - Destroying a nest strips `NestGuardian` from every creature that
    pointed at it, and a subsequent wander move is no longer
    radius-constrained.
  - Killing a guardian queues exactly one respawn, and it spawns after
    exactly `NEST_RESPAWN_TICKS` ticks, not before.
  - Taming a guardian (successful decompile) also queues a respawn.
  - Killing/taming a guardian whose nest was already destroyed queues
    nothing (no panic, no orphaned respawn).
  - `raid_check` never selects a `Nest` entity as a target, even when
    it's the only `Durability`-holder in the world.
  - A species with `can_nest: false` (the default) never produces a nest,
    regardless of `NEST_SPAWN_CHANCE`.

## Changelog

Per this project's convention, `README.md` gets a dated, newest-first
changelog entry once this feature ships (e.g. "Wild creature nests: some
species now guard a stationary nest that respawns fallen guardians until
the nest itself is destroyed.").
