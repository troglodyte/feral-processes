# Wild Creature Nests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Certain wild-creature species can spawn as a stationary **Nest**
that keeps 2-5 guardians tethered within 5 tiles, respawns a fallen
guardian 10 ticks later, and stops doing either once the nest itself is
destroyed by the player.

**Architecture:** Nest and NestGuardian are new ECS components layered
onto the existing wild-creature spawn/wander/battle machinery
(`crates/engine/src`). Nest destruction is a direct bump-attack in
`move_player`, not routed through the existing `BattleState` battle
screen, because that machinery is built entirely around `Creature`/species
assumptions (decompile odds, retaliation, companion abilities) that don't
apply to a nest. Guardian respawning is a `Game` method called directly
from `tick_inner` (like `structure_regen`/`raid_check`), not a
`Schedule`-registered system, because it needs `spawn_wild_creature` and
friends, which are `Game` methods unreachable from a bevy system function.

**Tech Stack:** Rust, `bevy_ecs` (standalone, headless ECS — no Bevy app),
`ron` for species/structure asset files, `rand`/`RngExt` for `GameRng`.

## Global Constraints

- Every new `SpeciesDef` field must be `#[serde(default)]` so existing
  `.ron` files (including mods) keep parsing unchanged.
- `assets/species/README.md` must be updated in the same change as the
  `SpeciesDef` schema change (project rule — see `CLAUDE.md`).
- No hardcoded species list in Rust for nest eligibility — it's a
  per-species data flag (`can_nest`), moddable like everything else in
  `assets/species/`.
- Bosses never nest (`try_spawn_habitat_creature` only rolls a nest for
  the *ordinary*, non-boss pick).
- `README.md`'s Changelog needs a new bullet under the existing
  `### 2026-07-20` section once this ships (project convention — dated,
  newest-first entries).
- **Explicitly out of scope for this plan:** save/load persistence for
  `Nest`/`NestGuardian`. `save.rs` builds `SaveData` from explicit
  `CreatureSave`/`StructureSave` queries (`With<Creature>`,
  `With<Structure>`) — a `Nest` entity has neither component, so it is
  silently *not* captured today, and a `NestGuardian` creature reloads as
  an ordinary, untethered wild creature (its `NestGuardian` tag is not
  serialized). This is a known, accepted limitation, not an oversight —
  full persistence is a reasonable fast-follow but adds real scope
  (save-format versioning, `SaveData` schema changes) beyond what this
  feature's design doc calls for.
- Run `cargo test` at the workspace root before considering any task
  done — this codebase has many RNG-seeded tests, and this feature adds a
  new conditional RNG draw into `try_spawn_habitat_creature` (mirroring
  the existing `BOSS_SPAWN_CHANCE` pattern), which can shift downstream
  seeded outcomes for any existing test whose seed happens to roll a
  nest-eligible species. If that happens, fix the affected test's
  expectations — don't weaken the design to dodge it.

---

## File Structure

- `assets/species/README.md` — schema doc, gets the new `can_nest` field
  documented (Task 1).
- `crates/engine/src/species.rs` — `SpeciesDef.can_nest` field + test
  (Task 1).
- `crates/engine/src/components.rs` — new `Nest`, `NestGuardian`
  components (Task 2).
- `crates/engine/src/lib.rs` — new constants, `raid_check` exclusion,
  `entity_label` branch (Task 2); `spawn_wild_creature` signature change,
  `spawn_nest`/`spawn_nest_guardian`, `try_spawn_habitat_creature`
  integration (Task 3); `move_player`, `find_nest_at`, `attack_nest`
  (Task 5); `finish_front_pack_member`, `battle_decompile`,
  `nest_respawn_tick`, `tick_inner` hook (Task 6). All tests live in
  `lib.rs`'s existing `#[cfg(test)] mod tests`.
- `crates/engine/src/systems.rs` — `wander_ai_system` tether check
  (Task 4).
- `assets/species/scrapper.ron`, `worm.ron`, `wraith.ron`, `trojan.ron` —
  flip `can_nest: true` (Task 7).
- `README.md` — changelog bullet (Task 7).

---

### Task 1: `SpeciesDef.can_nest` schema field

**Files:**
- Modify: `crates/engine/src/species.rs:97-152` (the `SpeciesDef` struct)
- Modify: `assets/species/README.md`
- Test: `crates/engine/src/species.rs` (existing `#[cfg(test)] mod tests`
  block, ~line 216)

**Interfaces:**
- Produces: `SpeciesDef::can_nest: bool` — every other task that reads
  species data (`Game::try_spawn_habitat_creature` in Task 3) checks this
  field via `species_db.get(id).is_some_and(|s| s.can_nest)`.

- [ ] **Step 1: Add the field to `SpeciesDef`**

In `crates/engine/src/species.rs`, add this field right after
`growth_multiplier` (currently the last field, ending at line 147):

```rust
    /// Whether this species can spawn as a Nest — a stationary,
    /// destructible object that keeps 2-5 guardians of this species
    /// tethered around it and respawns any that are killed/tamed, until
    /// the nest itself is destroyed (see `components::Nest`,
    /// `Game::try_spawn_habitat_creature`). `#[serde(default)]` so
    /// existing species files (including mods) without this field keep
    /// parsing as non-nesting, same as before this field existed. Never
    /// applies to a boss species regardless of this flag — the habitat
    /// spawn roll only ever considers it for the ordinary (non-boss)
    /// pick.
    #[serde(default)]
    pub can_nest: bool,
```

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/engine/src/species.rs`
(after `base_roster_growth_multiplier_rises_with_difficulty_tier`):

```rust
    #[test]
    fn can_nest_defaults_to_false_for_species_files_that_omit_it() {
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
        assert!(warnings.is_empty(), "species assets should all load cleanly: {warnings:?}");

        // None of the base roster's .ron files set can_nest yet (Task 7
        // flips a few to true) — at this point in the plan every species
        // must still default to false.
        assert!(
            db.all().all(|s| !s.can_nest),
            "can_nest should default to false until Task 7 opts specific species in"
        );
    }
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine can_nest_defaults_to_false`
Expected: PASS (the `#[serde(default)]` field defaults to `false`, and no
`.ron` file sets it yet).

- [ ] **Step 4: Document the field in the schema README**

In `assets/species/README.md`, add this right after the
`growth_multiplier` block (the last field before the closing `)` at
line 99), before the closing `)`:

```
    // Optional; can be left out entirely (defaults to false). If true,
    // this species can spawn as a Nest instead of an ordinary lone
    // creature/pack during habitat spawning: a stationary, destructible
    // object that keeps 2-5 guardians of this species tethered within 5
    // tiles of it, respawning any that are killed or tamed 10 ticks
    // later, until the nest itself is destroyed (walk into it to attack
    // it — it never attacks back). Never applies to a boss species,
    // regardless of this flag.
    can_nest: false,
```

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/species.rs assets/species/README.md
git commit -m "$(cat <<'EOF'
Add can_nest schema field to SpeciesDef

Opt-in per species, defaults to false so every existing .ron file
(including mods) keeps parsing unchanged.
EOF
)"
```

---

### Task 2: `Nest`/`NestGuardian` components, constants, raid exclusion, label

**Files:**
- Modify: `crates/engine/src/components.rs` (add new components near
  `Durability`, ~line 458)
- Modify: `crates/engine/src/lib.rs:158-211` (constants block, right
  after `BOSS_PORTAL_FRAGMENT_DROP`)
- Modify: `crates/engine/src/lib.rs:24-30` (the `components::{...}`
  import list)
- Modify: `crates/engine/src/lib.rs:2981-3000` (`raid_check`'s target
  query)
- Modify: `crates/engine/src/lib.rs:3578-3590` (`entity_label`)
- Test: `crates/engine/src/lib.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from Task 1 directly (this task is components/
  constants/plumbing only).
- Produces:
  - `components::Nest { species: SpeciesId, pending_respawns: Vec<u32> }`
  - `components::NestGuardian { nest: Entity }`
  - `lib.rs` constants: `NEST_SPAWN_CHANCE: f64`,
    `pub(crate) const NEST_TETHER_RADIUS: i32` (must be `pub(crate)` —
    Task 4 reads it from `systems.rs`), `NEST_GUARDIAN_MIN: u32`,
    `NEST_GUARDIAN_MAX: u32`, `NEST_RESPAWN_TICKS: u32`,
    `NEST_DURABILITY: u32`.
  - `raid_check` never selects a `Nest` entity as a target.
  - `entity_label` returns `"{species name} Nest"` for a `Nest` entity.

- [ ] **Step 1: Add the components**

In `crates/engine/src/components.rs`, add after the `Durability` struct
(after line 462, before the `Temporary` doc comment at line 464):

```rust
/// A stationary spawner for a wild species — see the nests feature (spec:
/// `docs/superpowers/specs/2026-07-20-nests-design.md`). Present on the
/// nest entity itself, which also carries `Position`, `Glyph`, and
/// `Durability` (all reused as-is — a nest is destroyed the same way a
/// structure is, just via a direct bump-attack instead of a raid).
#[derive(Component, Clone, Debug)]
pub struct Nest {
    pub species: SpeciesId,
    /// Ticks remaining until each queued replacement guardian spawns —
    /// one entry per guardian currently missing from the nest's original
    /// count (see `systems`-adjacent `Game::nest_respawn_tick`). Emptied
    /// naturally once every missing guardian is back.
    pub pending_respawns: Vec<u32>,
}

/// Tags a wild creature as tethered to a `Nest` — see
/// `systems::wander_ai_system`'s radius check. Removed (not the
/// creature) when its nest is destroyed (`Game::attack_nest`) or when the
/// creature itself is killed/tamed, at which point it either despawns or
/// resumes ordinary untethered behavior.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}
```

- [ ] **Step 2: Add the constants**

In `crates/engine/src/lib.rs`, add right after `BOSS_PORTAL_FRAGMENT_DROP`
(currently ending at line 162, before the `DIFFICULTY_EASY_MAX` doc
comment at line 164):

```rust
/// Chance a habitat spawn roll (see `Game::try_spawn_habitat_creature`)
/// produces a Nest instead of an ordinary pack, for a species that has
/// `SpeciesDef::can_nest` set. Only rolled at all when `can_nest` is
/// true, mirroring how `BOSS_SPAWN_CHANCE` is only rolled when a boss
/// candidate exists — keeps the extra RNG draw out of the common
/// non-nesting path entirely.
const NEST_SPAWN_CHANCE: f64 = 0.06;

/// Chebyshev distance a `NestGuardian` may wander from its `Nest` — see
/// `systems::wander_ai_system`. `pub(crate)` so `systems.rs` (a sibling
/// module) can read it too.
pub(crate) const NEST_TETHER_RADIUS: i32 = 5;

/// Inclusive range of guardians a freshly spawned `Nest` starts with —
/// see `Game::spawn_nest`.
const NEST_GUARDIAN_MIN: u32 = 2;
const NEST_GUARDIAN_MAX: u32 = 5;

/// Ticks between a guardian's death/taming and its replacement spawning
/// — see `Game::nest_respawn_tick`.
const NEST_RESPAWN_TICKS: u32 = 10;

/// A Nest's starting/max `Durability` — double the default structure
/// durability (`structures::default_durability`), since it's meant to
/// take real, sustained effort to clear, not a single lucky hit.
const NEST_DURABILITY: u32 = 60;
```

- [ ] **Step 3: Import the new components**

In `crates/engine/src/lib.rs:24-30`, add `Nest` and `NestGuardian` to the
existing `components::{...}` import list (alphabetical among the
existing names):

```rust
use components::{
    ActiveBuff, ActiveStatus, BuffKind, Creature, CustomName, Decompiler, Durability, Equipment,
    EquippedItem, Experience, Glyph, GlyphColor, Hostile, Inventory, ItemFusions,
    MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, Needs, Nest, NestGuardian, PassiveProcessor, Perks,
    Player, PlayerBuff, Position, Potential, ResourceNode, Stats, StatusEffects, StatusKind,
    Structure, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
```

- [ ] **Step 4: Exclude `Nest` from raid targeting**

In `crates/engine/src/lib.rs`, change `raid_check`'s target query
(currently at line 2990):

```rust
            let mut query = self.world.query_filtered::<Entity, With<Durability>>();
```

to:

```rust
            let mut query = self
                .world
                .query_filtered::<Entity, (With<Durability>, Without<Nest>)>();
```

- [ ] **Step 5: Add the `Nest` branch to `entity_label`**

In `crates/engine/src/lib.rs`, change `entity_label` (currently lines
3578-3590):

```rust
    fn entity_label(&self, entity: Entity) -> String {
        if let Some(name) = self.creature_name(entity) {
            self.zone_tagged_name(entity, name)
        } else if let Some(s) = self.world.get::<Structure>(entity) {
            self.world
                .resource::<StructureDb>()
                .get(&s.kind)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| s.kind.clone())
        } else if let Some(nest) = self.world.get::<Nest>(entity) {
            let species_name = self
                .world
                .resource::<SpeciesDb>()
                .get(&nest.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| nest.species.clone());
            format!("{species_name} Nest")
        } else {
            "You".to_string()
        }
    }
```

- [ ] **Step 6: Write the failing test for the raid exclusion**

Add to `lib.rs`'s `#[cfg(test)] mod tests` (a good spot is right after
`successful_decompile_removes_wander_ai_so_the_tamed_creature_stops_roaming`,
~line 5394):

```rust
    #[test]
    fn raid_check_never_targets_a_nest_even_as_the_only_durability_holder() {
        let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Strip every other Durability holder so a Nest would be the only
        // possible target if it weren't explicitly excluded.
        let existing: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Durability>>();
            query.iter(&game.world).collect()
        };
        for e in existing {
            game.world.despawn(e);
        }
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 10, y: 10 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        for _ in 0..500 {
            game.raid_check();
        }

        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY,
            "a Nest must never take raid damage, even when it's the only Durability holder"
        );
    }
```

- [ ] **Step 7: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine raid_check_never_targets_a_nest -- --nocapture`
Expected: FAIL before Step 4's fix is applied (or a compile error if you
haven't done Step 4 yet — apply Steps 1-5 first, then run this to
confirm PASS instead; there's no meaningful "red" state here since the
fix and the test are naturally written together. If you want to see it
fail, temporarily revert the `Without<Nest>` filter back to
`With<Durability>` alone, run the test, confirm FAIL, then re-apply the
fix).

- [ ] **Step 8: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine raid_check_never_targets_a_nest`
Expected: PASS

- [ ] **Step 9: Run the full engine test suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS (this task only adds code paths gated behind types nothing
else constructs yet, so nothing existing should change behavior).

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/components.rs crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
Add Nest/NestGuardian components and exclude nests from raids

Nests reuse Durability like a structure but must never be picked as a
raid target — raids represent hostile programs attacking the player's
own base, and a nest isn't part of it.
EOF
)"
```

---

### Task 3: Spawning — `spawn_nest`, `spawn_nest_guardian`, habitat roll integration

**Files:**
- Modify: `crates/engine/src/lib.rs:2819-2850` (`spawn_wild_creature`)
- Modify: `crates/engine/src/lib.rs:3076-3138`
  (`try_spawn_habitat_creature`)
- Test: `crates/engine/src/lib.rs`

**Interfaces:**
- Consumes: `Nest`, `NestGuardian` (Task 2); `SpeciesDef::can_nest`
  (Task 1); `NEST_SPAWN_CHANCE`, `NEST_GUARDIAN_MIN`, `NEST_GUARDIAN_MAX`,
  `NEST_DURABILITY`, `NEST_TETHER_RADIUS` (Task 2).
- Produces:
  - `fn spawn_wild_creature(&mut self, species_id: &str, x: i32, y: i32) -> Option<Entity>`
    (signature change: was `-> ()`; `None` only when `species_id` isn't in
    `SpeciesDb`, same silent-skip behavior as before, just now observable).
  - `fn spawn_nest(&mut self, species_id: &str, x: i32, y: i32)`
  - `fn spawn_nest_guardian(&mut self, nest: Entity, species_id: &str, nest_x: i32, nest_y: i32)`
    — Task 6's `nest_respawn_tick` calls this too.

- [ ] **Step 1: Change `spawn_wild_creature`'s return type**

In `crates/engine/src/lib.rs`, change (currently lines 2819-2850):

```rust
    fn spawn_wild_creature(&mut self, species_id: &str, x: i32, y: i32) {
        let Some(species) = self.world.resource::<SpeciesDb>().get(species_id).cloned() else {
            return;
        };
        let zone_level = self.world.resource::<ZoneLevel>();
        let mult = zone_level.stat_multiplier() as f32;
        let zone = zone_level.0;
        let dist_mult = self.distance_stat_multiplier(x, y);
        let potential = self.roll_potential();
        let scale = |base: i32, roll: f32| ((base as f32) * mult * dist_mult * roll).round() as i32;
        self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: scale(species.base_hp, potential.hp_roll),
                max_hp: scale(species.base_hp, potential.hp_roll),
                atk: scale(species.base_atk, potential.atk_roll),
                def: scale(species.base_def, potential.def_roll),
            },
            potential,
            Hostile,
            WanderAi::default(),
            ZonePortal(zone),
            StatusEffects::default(),
        ));
    }
```

to:

```rust
    /// Spawns a wild creature of `species_id` at `(x, y)`, returning its
    /// `Entity` — `None` only if `species_id` isn't in `SpeciesDb` (every
    /// real call site passes an id it already validated against
    /// `SpeciesDb`, so this is a defensive no-op path, not an expected
    /// outcome). `spawn_nest_guardian` uses the returned entity to attach
    /// `NestGuardian`.
    fn spawn_wild_creature(&mut self, species_id: &str, x: i32, y: i32) -> Option<Entity> {
        let species = self.world.resource::<SpeciesDb>().get(species_id).cloned()?;
        let zone_level = self.world.resource::<ZoneLevel>();
        let mult = zone_level.stat_multiplier() as f32;
        let zone = zone_level.0;
        let dist_mult = self.distance_stat_multiplier(x, y);
        let potential = self.roll_potential();
        let scale = |base: i32, roll: f32| ((base as f32) * mult * dist_mult * roll).round() as i32;
        Some(
            self.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Position { x, y },
                    Glyph {
                        ch: species.glyph,
                        color: species.color,
                    },
                    Stats {
                        hp: scale(species.base_hp, potential.hp_roll),
                        max_hp: scale(species.base_hp, potential.hp_roll),
                        atk: scale(species.base_atk, potential.atk_roll),
                        def: scale(species.base_def, potential.def_roll),
                    },
                    potential,
                    Hostile,
                    WanderAi::default(),
                    ZonePortal(zone),
                    StatusEffects::default(),
                ))
                .id(),
        )
    }
```

The two existing call sites (`try_spawn_habitat_creature`'s
`self.spawn_wild_creature(&pick, gx, gy);` and a test at what's currently
line 6208) call this as a bare statement and already discard the return
value — an `Option<Entity>` return doesn't break either of them.

- [ ] **Step 2: Add `spawn_nest` and `spawn_nest_guardian`**

Add these two new methods right after `spawn_wild_creature`:

```rust
    /// Spawns a `Nest` for `species_id` at `(x, y)`, plus an initial
    /// `NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX` guardians clustered within
    /// `NEST_TETHER_RADIUS` of it. See the nests design doc
    /// (`docs/superpowers/specs/2026-07-20-nests-design.md`).
    fn spawn_nest(&mut self, species_id: &str, x: i32, y: i32) {
        let Some(species) = self.world.resource::<SpeciesDb>().get(species_id).cloned() else {
            return;
        };
        let nest = self
            .world
            .spawn((
                Nest {
                    species: species.id.clone(),
                    pending_respawns: Vec::new(),
                },
                Position { x, y },
                Glyph {
                    ch: 'N',
                    color: species.color,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian_count = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX)
        };
        for _ in 0..guardian_count {
            self.spawn_nest_guardian(nest, species_id, x, y);
        }
    }

    /// Spawns one `species_id` wild creature tethered to `nest`, at a
    /// random offset within `NEST_TETHER_RADIUS` of `(nest_x, nest_y)` —
    /// used both for a nest's initial guardians (`spawn_nest`) and for
    /// respawns (`nest_respawn_tick`). Walkability isn't rechecked for the
    /// offset tile, matching the existing looseness
    /// `try_spawn_habitat_creature` already has for pack members.
    fn spawn_nest_guardian(&mut self, nest: Entity, species_id: &str, nest_x: i32, nest_y: i32) {
        let (gx, gy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (
                nest_x + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
                nest_y + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
            )
        };
        if let Some(guardian) = self.spawn_wild_creature(species_id, gx, gy) {
            self.world.entity_mut(guardian).insert(NestGuardian { nest });
        }
    }
```

- [ ] **Step 3: Wire the nest roll into `try_spawn_habitat_creature`**

In `crates/engine/src/lib.rs`, `try_spawn_habitat_creature` currently
reads (lines 3107-3121):

```rust
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..pool.len());
            pool[idx].clone()
        };
        // Bosses always spawn alone — packs are an ordinary-encounter
        // mechanic, not something to stack onto an already-tough boss
        // fight.
        let group_size = if spawn_boss {
            1
        } else {
            let max_pack = self.max_pack_size(x, y);
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(1..=max_pack)
        };
```

Insert the nest roll between the `pick` and the `group_size` block:

```rust
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..pool.len());
            pool[idx].clone()
        };

        // A nest takes the tile's spawn slot instead of an ordinary pack,
        // same "rare special outcome" shape as the boss roll above — but
        // only ever considered for the non-boss pick, and only for a
        // species that opted in via `SpeciesDef::can_nest`. The RNG draw
        // only happens when `can_nest` is true, so this never shifts the
        // RNG sequence for the (overwhelmingly common) non-nesting case.
        if !spawn_boss {
            let can_nest = self
                .world
                .resource::<SpeciesDb>()
                .get(&pick)
                .is_some_and(|s| s.can_nest);
            let spawn_nest_roll = can_nest && {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(NEST_SPAWN_CHANCE)
            };
            if spawn_nest_roll {
                self.spawn_nest(&pick, x, y);
                return true;
            }
        }

        // Bosses always spawn alone — packs are an ordinary-encounter
        // mechanic, not something to stack onto an already-tough boss
        // fight.
        let group_size = if spawn_boss {
            1
        } else {
            let max_pack = self.max_pack_size(x, y);
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(1..=max_pack)
        };
```

- [ ] **Step 4: Write the failing test**

Add to `lib.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn spawn_nest_creates_a_tethered_guardian_cluster() {
        let mut game = Game::new(601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.spawn_nest("scrapper", 30, 30);

        let nests: Vec<(Entity, Position)> = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            query
                .iter(&game.world)
                .map(|(e, _, p)| (e, *p))
                .collect()
        };
        assert_eq!(nests.len(), 1, "spawn_nest should create exactly one Nest entity");
        let (nest, nest_pos) = nests[0];
        assert_eq!(nest_pos, Position { x: 30, y: 30 });
        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY
        );

        let guardians: Vec<Position> = {
            let mut query = game.world.query::<(&NestGuardian, &Position)>();
            query
                .iter(&game.world)
                .filter(|(g, _)| g.nest == nest)
                .map(|(_, p)| *p)
                .collect()
        };
        assert!(
            guardians.len() >= NEST_GUARDIAN_MIN as usize
                && guardians.len() <= NEST_GUARDIAN_MAX as usize,
            "expected {}..={} guardians, got {}",
            NEST_GUARDIAN_MIN,
            NEST_GUARDIAN_MAX,
            guardians.len()
        );
        for pos in guardians {
            let dist = (pos.x - 30).abs().max((pos.y - 30).abs());
            assert!(
                dist <= NEST_TETHER_RADIUS,
                "guardian spawned {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
            );
        }
    }
```

- [ ] **Step 5: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine spawn_nest_creates_a_tethered_guardian_cluster -- --nocapture`
Expected: FAIL with a compile error (`no method named spawn_nest`) before
Steps 1-3 are applied. Apply them, then re-run.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine spawn_nest_creates_a_tethered_guardian_cluster`
Expected: PASS

- [ ] **Step 7: Run the full engine test suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS. `can_nest` is still `false` for every real species until
Task 7, so `try_spawn_habitat_creature`'s new branch is dead code on any
real habitat roll right now — no existing test should be affected.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
Add nest spawning: spawn_nest, spawn_nest_guardian, habitat roll hook

spawn_wild_creature now returns the spawned Entity so a guardian can be
tagged with NestGuardian right after creation.
EOF
)"
```

---

### Task 4: Tether enforcement in `wander_ai_system`

**Files:**
- Modify: `crates/engine/src/systems.rs:1-83`
- Test: `crates/engine/src/lib.rs`

**Interfaces:**
- Consumes: `Nest`, `NestGuardian` (Task 2), `NEST_TETHER_RADIUS` (Task 2,
  `pub(crate)`), `spawn_nest` (Task 3, used by the test to set up a
  scenario).
- Produces: `wander_ai_system` now refuses any move that would take a
  `NestGuardian` beyond `NEST_TETHER_RADIUS` of its nest's `Position`.
  Behavior for every other entity (no `NestGuardian`) is unchanged.

- [ ] **Step 1: Update imports**

In `crates/engine/src/systems.rs`, change the top-of-file imports
(currently lines 1-13):

```rust
use bevy_ecs::prelude::*;
use rand::RngExt;

use crate::NEST_TETHER_RADIUS;
use crate::components::{
    Creature, Experience, Inventory, Needs, Nest, NestGuardian, PassiveProcessor, Perks, Player,
    Position, Potential, ResourceNode, Stats, Structure, Tamed, Task, TaskKind, WanderAi,
};
use crate::perks::Perk;
use crate::progression;
use crate::resources::{GameRng, MessageKind, MessageLog};
use crate::species::SpeciesDb;
use crate::structures::StructureDb;
use crate::world::WorldMap;
```

- [ ] **Step 2: Add the tether check**

Change `wander_ai_system` (currently lines 61-83):

```rust
pub fn wander_ai_system(
    mut query: Query<(&mut Position, &mut WanderAi), Without<Player>>,
    mut world: ResMut<WorldMap>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut ai) in &mut query {
        if ai.cooldown > 0 {
            ai.cooldown -= 1;
            continue;
        }
        ai.cooldown = rng.0.random_range(2..6);
        let dx = rng.0.random_range(-1..=1);
        let dy = rng.0.random_range(-1..=1);
        if dx == 0 && dy == 0 {
            continue;
        }
        let (nx, ny) = (pos.x + dx, pos.y + dy);
        if world.tile(nx, ny).walkable {
            pos.x = nx;
            pos.y = ny;
        }
    }
}
```

to:

```rust
pub fn wander_ai_system(
    mut query: Query<(&mut Position, &mut WanderAi, Option<&NestGuardian>), Without<Player>>,
    nests: Query<&Position, (With<Nest>, Without<WanderAi>)>,
    mut world: ResMut<WorldMap>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut ai, guardian) in &mut query {
        if ai.cooldown > 0 {
            ai.cooldown -= 1;
            continue;
        }
        ai.cooldown = rng.0.random_range(2..6);
        let dx = rng.0.random_range(-1..=1);
        let dy = rng.0.random_range(-1..=1);
        if dx == 0 && dy == 0 {
            continue;
        }
        let (nx, ny) = (pos.x + dx, pos.y + dy);
        if let Some(guardian) = guardian
            && let Ok(nest_pos) = nests.get(guardian.nest)
        {
            let dist = (nx - nest_pos.x).abs().max((ny - nest_pos.y).abs());
            if dist > NEST_TETHER_RADIUS {
                continue;
            }
        }
        if world.tile(nx, ny).walkable {
            pos.x = nx;
            pos.y = ny;
        }
    }
}
```

Note: if this fails to compile with a query-data-access-conflict panic or
error (Bevy couldn't prove `query` and `nests` are disjoint), fall back to
combining them with `bevy_ecs::prelude::ParamSet` — but try this form
first; `nests`' `Without<WanderAi>` filter should be enough for Bevy's
static analysis, since `query` requires `&mut WanderAi` (i.e. the
component must be present) on every matched entity.

- [ ] **Step 3: Write the failing test**

Add to `lib.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn guardian_never_wanders_beyond_the_nest_tether_radius() {
        let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.spawn_nest("scrapper", 40, 40);

        let (nest, nest_pos) = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            let (e, _, p) = query.iter(&game.world).next().expect("nest should exist");
            (e, *p)
        };
        let guardians: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&game.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        assert!(!guardians.is_empty());

        for _ in 0..200 {
            game.tick();
            for &guardian in &guardians {
                let pos = *game.world.get::<Position>(guardian).unwrap();
                let dist = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
                assert!(
                    dist <= NEST_TETHER_RADIUS,
                    "guardian wandered {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
                );
            }
        }
    }
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine guardian_never_wanders_beyond_the_nest_tether_radius -- --nocapture`
Expected: FAIL before Steps 1-2 are applied (compile error — `Nest`/
`NestGuardian` not yet referenced in `systems.rs`, or the tether isn't
enforced yet). Apply Steps 1-2, then re-run.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine guardian_never_wanders_beyond_the_nest_tether_radius`
Expected: PASS

- [ ] **Step 6: Run the full engine test suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/systems.rs
git commit -m "$(cat <<'EOF'
Enforce nest tether radius in wander_ai_system

A NestGuardian's candidate move is rejected once it would exceed
NEST_TETHER_RADIUS from its nest, the same way an unwalkable tile is.
EOF
)"
```

---

### Task 5: Bump-attack destruction (`move_player`, `find_nest_at`, `attack_nest`)

**Files:**
- Modify: `crates/engine/src/lib.rs:965-994` (`move_player`)
- Test: `crates/engine/src/lib.rs`

**Interfaces:**
- Consumes: `Nest`, `NestGuardian` (Task 2), `entity_label`'s `Nest`
  branch (Task 2), `effective_atk`, `battle::compute_damage` (existing).
- Produces: `fn find_nest_at(&mut self, x: i32, y: i32) -> Option<Entity>`,
  `fn attack_nest(&mut self, nest: Entity)`. Walking into a `Nest` tile no
  longer just blocks movement — it deals damage, and on destruction
  strips `NestGuardian` from every creature tethered to it.

- [ ] **Step 1: Add `find_nest_at` and `attack_nest`**

Add these two methods right after `find_wild_creature_at` (currently
lines 2687-2695):

```rust
    /// Finds a `Nest` at `(x, y)`, if any — checked in `move_player`
    /// before the ordinary blocking-structure check, so walking into a
    /// nest tile attacks it instead of just being blocked.
    fn find_nest_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self.world.query_filtered::<(Entity, &Position), With<Nest>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Deals one hit of the player's `effective_atk` (against no defense
    /// — a nest has none, only a `Durability` pool) to `nest`. A nest
    /// never retaliates, unlike an ordinary wild-creature encounter — see
    /// the nests design doc for why this deliberately isn't routed
    /// through `BattleState`. Destroying it strips `NestGuardian` from
    /// every creature tethered to it (they resume ordinary wandering) and
    /// despawns the nest, which implicitly cancels anything left in its
    /// `Nest::pending_respawns`.
    fn attack_nest(&mut self, nest: Entity) {
        let player = self.player_entity();
        let label = self.entity_label(nest);
        let dmg = battle::compute_damage(self.effective_atk(player), 0, 5) as u32;
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
            let guardians: Vec<Entity> = {
                let mut query = self.world.query::<(Entity, &NestGuardian)>();
                query
                    .iter(&self.world)
                    .filter(|(_, g)| g.nest == nest)
                    .map(|(e, _)| e)
                    .collect()
            };
            for guardian in guardians {
                self.world.entity_mut(guardian).remove::<NestGuardian>();
            }
            self.world.despawn(nest);
        } else {
            self.log(format!("You unleash a data strike into the {label} for {dmg} damage."));
        }
    }
```

- [ ] **Step 2: Wire it into `move_player`**

Change `move_player` (currently lines 965-994):

```rust
    pub fn move_player(&mut self, dx: i32, dy: i32) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let (nx, ny) = (pos.x + dx, pos.y + dy);

        if let Some(target) = self.find_wild_creature_at(nx, ny) {
            let pack = self.gather_pack(target);
            self.start_battle(pack);
            self.tick();
            return;
        }
        if self.find_zone_portal_at(nx, ny).is_some() {
            self.enter_next_zone();
            self.tick();
            return;
        }
        if self.find_blocking_structure_at(nx, ny).is_some() {
            return;
        }
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
        }
        self.tick();
    }
```

to (only the new block after the wild-creature check is added):

```rust
    pub fn move_player(&mut self, dx: i32, dy: i32) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let (nx, ny) = (pos.x + dx, pos.y + dy);

        if let Some(target) = self.find_wild_creature_at(nx, ny) {
            let pack = self.gather_pack(target);
            self.start_battle(pack);
            self.tick();
            return;
        }
        if let Some(nest) = self.find_nest_at(nx, ny) {
            self.attack_nest(nest);
            self.tick();
            return;
        }
        if self.find_zone_portal_at(nx, ny).is_some() {
            self.enter_next_zone();
            self.tick();
            return;
        }
        if self.find_blocking_structure_at(nx, ny).is_some() {
            return;
        }
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
        }
        self.tick();
    }
```

- [ ] **Step 3: Write the failing test**

Add to `lib.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians() {
        let mut game = Game::new(603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Position>(player).unwrap().x = 49;
        game.world.get_mut::<Position>(player).unwrap().y = 50;

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability { hp: 5, max_hp: 5 },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 52, y: 52 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Player's base ATK (6) vs. 0 defense, move_power 5 → well over 5
        // damage, so one bump is enough to destroy a 5-HP nest.
        game.move_player(1, 0);

        assert!(
            game.world.get::<Nest>(nest).is_none(),
            "nest should be destroyed by one bump"
        );
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "guardian should lose its NestGuardian tether once the nest is destroyed"
        );
    }
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians -- --nocapture`
Expected: FAIL with a compile error (`find_nest_at`/`attack_nest` don't
exist) before Steps 1-2 are applied. Apply them, then re-run.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians`
Expected: PASS

- [ ] **Step 6: Run the full engine test suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
Add bump-attack nest destruction to move_player

Walking into a Nest deals damage instead of blocking movement or opening
a battle screen — a nest never retaliates, so no BattleState is needed.
Destroying it frees every tethered guardian.
EOF
)"
```

---

### Task 6: Respawn queuing and `nest_respawn_tick`

**Files:**
- Modify: `crates/engine/src/lib.rs:2024-2040`
  (`finish_front_pack_member`)
- Modify: `crates/engine/src/lib.rs:2395-2413` (`battle_decompile`'s
  success branch)
- Modify: `crates/engine/src/lib.rs:910-922` (`tick_inner`)
- Test: `crates/engine/src/lib.rs`

**Interfaces:**
- Consumes: `Nest`, `NestGuardian` (Task 2), `spawn_nest_guardian`
  (Task 3), `NEST_RESPAWN_TICKS` (Task 2).
- Produces: `fn nest_respawn_tick(&mut self)`, called from `tick_inner`.
  Killing or taming a `NestGuardian` whose nest still exists queues a
  replacement `NEST_RESPAWN_TICKS` ticks out.

- [ ] **Step 1: Queue a respawn on kill**

Change `finish_front_pack_member` (currently lines 2024-2040):

```rust
    fn finish_front_pack_member(&mut self, player: Entity) -> bool {
        let Some(front) = self.front_wild_creature() else {
            return true;
        };
        self.log("The rogue program crashes and deletes itself!");
        let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
        self.award_player_xp(player, wild_max_hp as u32);
        self.award_loot(player, front);
        self.world.despawn(front);
        if self.pop_front_pack_member() {
```

to:

```rust
    fn finish_front_pack_member(&mut self, player: Entity) -> bool {
        let Some(front) = self.front_wild_creature() else {
            return true;
        };
        self.log("The rogue program crashes and deletes itself!");
        let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
        self.award_player_xp(player, wild_max_hp as u32);
        self.award_loot(player, front);
        let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
        self.world.despawn(front);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        if self.pop_front_pack_member() {
```

(the rest of the function is unchanged).

- [ ] **Step 2: Queue a respawn on successful tame**

Change `battle_decompile`'s success branch (currently lines 2395-2402):

```rust
        if roll {
            let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
            self.world.entity_mut(front).remove::<(Hostile, WanderAi)>();
            self.world
                .entity_mut(front)
                .insert((Tamed { owner: player }, Experience::default()));
            self.log("ICE breached! The program now runs under your control.");
            self.award_player_xp(player, wild_max_hp as u32);
```

to:

```rust
        if roll {
            let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
            let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
            self.world
                .entity_mut(front)
                .remove::<(Hostile, WanderAi, NestGuardian)>();
            self.world
                .entity_mut(front)
                .insert((Tamed { owner: player }, Experience::default()));
            if let Some(nest) = nest
                && let Some(mut n) = self.world.get_mut::<Nest>(nest)
            {
                n.pending_respawns.push(NEST_RESPAWN_TICKS);
            }
            self.log("ICE breached! The program now runs under your control.");
            self.award_player_xp(player, wild_max_hp as u32);
```

(the rest of the function, including the failure branch below it, is
unchanged).

- [ ] **Step 3: Add `nest_respawn_tick`**

Add this method right after `structure_regen` (currently ending at
line 2955, before `raid_check`):

```rust
    /// Advances every `Nest`'s `pending_respawns` countdown by one tick,
    /// spawning a replacement guardian for each entry that reaches 0 (a
    /// nest can have more than one entry reach 0 on the same tick, e.g.
    /// two guardians killed together, so this spawns once per ready
    /// entry, not once per nest). Called directly from `tick_inner` —
    /// not registered on `self.schedule` — because it needs
    /// `spawn_nest_guardian`, a `Game` method unreachable from a bevy
    /// system function.
    fn nest_respawn_tick(&mut self) {
        let ready: Vec<(Entity, SpeciesId, Position, usize)> = {
            let mut query = self.world.query::<(Entity, &mut Nest, &Position)>();
            query
                .iter_mut(&mut self.world)
                .filter_map(|(entity, mut nest, pos)| {
                    for slot in nest.pending_respawns.iter_mut() {
                        *slot = slot.saturating_sub(1);
                    }
                    let ready_count = nest.pending_respawns.iter().filter(|&&t| t == 0).count();
                    if ready_count == 0 {
                        return None;
                    }
                    nest.pending_respawns.retain(|&t| t != 0);
                    Some((entity, nest.species.clone(), *pos, ready_count))
                })
                .collect()
        };
        for (nest, species, pos, count) in ready {
            for _ in 0..count {
                self.spawn_nest_guardian(nest, &species, pos.x, pos.y);
            }
        }
    }
```

This needs `SpeciesId` importable in `lib.rs` — add it to the existing
`species::{...}` import (currently line 38):

```rust
use species::{MoveDef, SpecialAbility, SpeciesDb, SpeciesDef, SpeciesId};
```

- [ ] **Step 4: Call it from `tick_inner`**

Change `tick_inner` (currently lines 910-922):

```rust
    fn tick_inner(&mut self, age_temporary: bool) {
        if self.is_game_over().is_some() {
            return;
        }
        self.maybe_spawn_wild_creature();
        self.schedule.run(&mut self.world);
        self.structure_regen();
        self.raid_check();
        if age_temporary {
            self.age_temporary_structures();
        }
        self.world.resource_mut::<GameClock>().tick += 1;
    }
```

to:

```rust
    fn tick_inner(&mut self, age_temporary: bool) {
        if self.is_game_over().is_some() {
            return;
        }
        self.maybe_spawn_wild_creature();
        self.schedule.run(&mut self.world);
        self.structure_regen();
        self.raid_check();
        self.nest_respawn_tick();
        if age_temporary {
            self.age_temporary_structures();
        }
        self.world.resource_mut::<GameClock>().tick += 1;
    }
```

- [ ] **Step 5: Write the failing tests**

Add to `lib.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn killing_a_guardian_respawns_a_replacement_after_exactly_the_respawn_delay() {
        let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 60, y: 60 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest },
                Position { x: 61, y: 61 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        // battle_attack's own kill-resolution path (finish_front_pack_member
        // returning true, the pack now empty) already calls self.tick() once
        // internally before returning — that tick already ran
        // nest_respawn_tick and decremented the entry we just pushed. So the
        // value observed here is NEST_RESPAWN_TICKS - 1, not the full delay.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "killing a guardian should queue one respawn"
        );

        let guardian_count = |game: &mut Game| -> usize {
            let mut query = game.world.query::<&NestGuardian>();
            query.iter(&game.world).filter(|g| g.nest == nest).count()
        };

        for _ in 0..(NEST_RESPAWN_TICKS - 2) {
            game.tick();
        }
        assert_eq!(
            guardian_count(&mut game),
            0,
            "no replacement should spawn before its delay elapses"
        );

        game.tick();
        assert_eq!(
            guardian_count(&mut game),
            1,
            "a replacement should spawn exactly when its delay elapses"
        );
    }

    #[test]
    fn taming_a_guardian_also_queues_a_respawn() {
        let mut game = Game::new(605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 70, y: 70 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 71, y: 71 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::IceBreaker, 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(guardian).is_some() {
                break;
            }
            game.battle_decompile();
        }

        assert!(game.world.get::<Tamed>(guardian).is_some());
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "a tamed creature should lose its nest tether"
        );
        // Same off-by-one as the kill test above: battle_decompile's
        // success path also calls self.tick() once internally before
        // returning, which already decremented the entry we just pushed.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "taming a guardian should also queue one respawn"
        );
    }

    #[test]
    fn killing_a_guardian_whose_nest_is_already_gone_queues_nothing() {
        let mut game = Game::new(606, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // A dangling nest Entity — never actually spawned, standing in
        // for "the nest was destroyed before this guardian died."
        let gone_nest = game.world.spawn_empty().id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest: gone_nest },
                Position { x: 80, y: 80 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        // Should not panic even though `gone_nest` has no Nest component.
        game.battle_attack();

        for _ in 0..(NEST_RESPAWN_TICKS + 5) {
            game.tick();
        }
        // Nothing to assert beyond "didn't panic" — there's no Nest left
        // to have queued anything on, and no new guardian entity for a
        // nonexistent nest.
    }
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine respawn -- --nocapture`
Expected: FAIL with compile errors (`nest_respawn_tick` doesn't exist,
`Nest`/`NestGuardian` not referenced yet in these call sites) before
Steps 1-4 are applied. Apply them, then re-run.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine respawn`
Run: `cargo test -p feral-processes-engine taming_a_guardian_also_queues_a_respawn`
Expected: PASS

- [ ] **Step 8: Run the full engine test suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
Queue and process nest guardian respawns

Killing or taming a NestGuardian whose nest still exists queues a
replacement NEST_RESPAWN_TICKS ticks out; nest_respawn_tick processes
the countdown once per game tick from tick_inner.
EOF
)"
```

---

### Task 7: Enable `can_nest` on real species, changelog, full verification

**Files:**
- Modify: `assets/species/scrapper.ron`
- Modify: `assets/species/worm.ron`
- Modify: `assets/species/wraith.ron`
- Modify: `assets/species/trojan.ron`
- Modify: `README.md` (changelog)
- Test: full workspace suite

**Interfaces:**
- Consumes: everything from Tasks 1-6.
- Produces: a playable feature — these four species can now spawn as
  nests during ordinary habitat rolls.

- [ ] **Step 1: Flip `can_nest: true` on four species**

In each of `assets/species/scrapper.ron`, `worm.ron`, `wraith.ron`,
`trojan.ron`, add `can_nest: true,` as the last field inside the `(...)`
block (after `growth_multiplier`, matching the schema order documented in
`assets/species/README.md`). Example for `scrapper.ron` — add this line
right before the file's closing `)`:

```ron
    can_nest: true,
```

Repeat for the other three files.

- [ ] **Step 2: Update the schema test's expectations**

The test added in Task 1
(`can_nest_defaults_to_false_for_species_files_that_omit_it`) currently
asserts *every* species has `can_nest == false`, which is no longer true.
Replace it in `crates/engine/src/species.rs` with:

```rust
    #[test]
    fn can_nest_is_set_only_for_the_intended_swarm_flavored_species() {
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
        assert!(warnings.is_empty(), "species assets should all load cleanly: {warnings:?}");

        let nesting: Vec<&str> = db
            .all()
            .filter(|s| s.can_nest)
            .map(|s| s.id.as_str())
            .collect();
        let mut nesting = nesting;
        nesting.sort();
        assert_eq!(nesting, vec!["scrapper", "trojan", "wraith", "worm"]);

        // No boss should ever be nest-eligible, regardless of this flag —
        // try_spawn_habitat_creature only rolls a nest for the non-boss
        // pick, but the data itself shouldn't set can_nest on a boss
        // either, to avoid a misleading .ron file.
        assert!(
            db.all().all(|s| !(s.is_boss && s.can_nest)),
            "no boss species should have can_nest set"
        );
    }
```

- [ ] **Step 3: Run the updated test**

Run: `cargo test -p feral-processes-engine can_nest_is_set_only_for`
Expected: PASS

- [ ] **Step 4: Add the changelog bullet**

In `README.md`, add a new bullet under the existing `### 2026-07-20`
heading (right after the heading, before the first existing bullet at
what's currently line 630):

```markdown
- **Wild creature nests**: Scrapper, Worm, Wraith, and Trojan can now
  spawn as a stationary Nest instead of an ordinary lone creature/pack —
  it keeps 2-5 guardians of its species tethered within 5 tiles, and any
  guardian that's killed or tamed is replaced 10 ticks later. Walk into
  the nest itself to attack it (it never attacks back); destroying it
  frees any surviving guardians to wander normally and stops further
  respawns. New species schema field: `can_nest` — see
  `assets/species/README.md`.
```

- [ ] **Step 5: Run the full workspace test suite**

Run: `cargo test`
Expected: PASS. If any *other* existing test fails, it's almost certainly
a downstream RNG-sequence shift from the new conditional roll in
`try_spawn_habitat_creature` (see the Global Constraints note at the top
of this plan) — a test that seeds a `Game` and then asserts something
about spawned creature counts/positions/stats near a biome where
Scrapper/Worm/Wraith/Trojan is a habitat match is the most likely
culprit. Fix the affected test's expected values (re-derive them by
running the test and reading the actual output), don't change the nest
spawn logic to avoid the shift — this is the same class of change the
original `BOSS_SPAWN_CHANCE` addition would have caused historically.

- [ ] **Step 6: Manually verify in the GUI**

Run: `cargo run -p feral-processes` (or whatever the launcher binary's
run command is per the project's `run` skill/README "Installing"
section), start a new game, and explore until a Nest (`N` glyph) is
found. Confirm: guardians are visible near it, walking into the nest
logs a strike message and reduces a visible durability bar, and repeated
bumps eventually destroy it and free any survivors. This step has no
automated pass/fail — note what you observed when reporting this task
done.

- [ ] **Step 7: Commit**

```bash
git add assets/species/scrapper.ron assets/species/worm.ron \
  assets/species/wraith.ron assets/species/trojan.ron \
  crates/engine/src/species.rs README.md
git commit -m "$(cat <<'EOF'
Enable nests on Scrapper, Worm, Wraith, and Trojan

Flips can_nest: true on the base roster's swarm-flavored Medium-tier
species, so the nest feature added in prior commits is actually
reachable during play.
EOF
)"
```

---

## Self-Review Notes

- **Spec coverage:** every section of
  `docs/superpowers/specs/2026-07-20-nests-design.md` maps to a task —
  data model → Task 1/2, spawning → Task 3, tethering → Task 4,
  destruction + raid exclusion → Task 2/5, respawns → Task 6, rendering
  (verified free, no task needed) and starter species/changelog → Task 7.
- **Type consistency:** `spawn_wild_creature`'s new `Option<Entity>`
  return is used consistently by `spawn_nest_guardian` (Task 3) and
  nowhere else changes its signature again. `Nest.pending_respawns:
  Vec<u32>` is read/written identically in `finish_front_pack_member`,
  `battle_decompile`, and `nest_respawn_tick` (Tasks 6). `NEST_TETHER_RADIUS`
  is `pub(crate)` from the moment it's introduced (Task 2), so Task 4's
  cross-module use compiles without a later visibility fix.
- **No placeholders:** every step above has literal, complete code —
  nothing deferred to "add validation" or "similar to Task N."
