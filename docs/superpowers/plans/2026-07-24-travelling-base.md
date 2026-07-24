# Travelling Base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make base-building worth doing by letting the base physically travel between zones and by putting its economy on the same doubling curve as every other progression axis.

**Architecture:** A Home stamps a 31×31 slab of a new `Biome::Platform` into `WorldMap`'s existing override overlay; that slab plus every structure standing on it is repositioned onto the new map on breach instead of being despawned. Danger scaling is re-measured from the slab's edge rather than its center. Worked-node payouts multiply by `ZoneLevel::stat_multiplier()`, and structures gain data-driven upgrade tiers as the per-zone material sink.

**Tech Stack:** Rust, `bevy_ecs` (standalone), `ron` for asset data, `bincode` for saves, `ratatui`/`crossterm` (TUI), `macroquad` (GUI).

**Design spec:** `docs/superpowers/specs/2026-07-24-travelling-base-design.md`

## Global Constraints

- Workspace is 5 crates; the engine's `Game` struct is the entire public API both renderers use via app-core. Renderers never touch the ECS `World`.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- `cargo test --workspace` is the final gate for every task, not just the new tests.
- New `StructureDef` / `SpeciesDef` / `ItemDef` fields MUST be `#[serde(default)]` so existing and third-party `.ron` files keep parsing.
- A malformed `.ron` file is skipped with a logged warning, never a panic.
- Any schema change updates the matching `assets/*/README.md` in the same task.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG. Background systems (habitat spawning, nests) will interfere with naive assertions — seed every `Game::new`.
- Comments explain *why*, never *what*.
- `SAVE_FORMAT_VERSION` is bumped exactly once for this whole branch, in Task 1. Later tasks add fields to `SaveData`/`StructureSave` without bumping again, because v9 has not shipped.
- Platform radius is `MAX_BUILD_DISTANCE_FROM_HOME` (lib.rs:236, value 15). Do NOT introduce a second constant — the platform is by definition exactly the buildable area.

## Shared test helpers

Several tasks below reference these by name. Add them to `mod tests` in
`crates/engine/src/lib.rs` the first time a task needs one. Existing helpers
already in that module and used unchanged: `test_assets_dir()`,
`place_home(game, dx, dy)` (lib.rs:5779 — grants the 5 Core Fragments
itself), and `spawn_tamed(game, x, y)` (a fully-formed tamed creature
carrying every component `systems::CronjobWorker` queries for).

```rust
/// How many of `id` the player is holding. `Game::player_entity` takes
/// `&self`, and `mod tests` is inside `lib.rs`, so both it and the private
/// component access below are reachable from tests.
fn count_item(game: &Game, id: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(id))
}

fn find_structure_by_kind(game: &mut Game, kind: &str) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
}

/// Runs exactly one completed gather cycle against a hand-built node
/// producing `resource`, optionally at `tier`, and returns how many units
/// landed in the player's inventory.
///
/// `level: None` means the node always yields (see
/// `systems::mining_success_chance`), which is what keeps every payout test
/// off the RNG — do not swap it for a seeded roll and retry.
fn run_one_full_gather_cycle(game: &mut Game, resource: &str, tier: Option<u32>) -> u32 {
    let worker = spawn_tamed(game, 10, 3);
    let mut structure = game.world.spawn((
        Structure {
            kind: "mining_node".to_string(),
        },
        Position { x: 3, y: 4 },
        ResourceNode {
            resource: ItemId::from(resource),
            amount: 5,
            capacity: 5,
            level: None,
        },
    ));
    if let Some(t) = tier {
        structure.insert(StructureTier(t));
    }
    let structure = structure.id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    let before = count_item(game, resource);
    game.tick();
    count_item(game, resource) - before
}
```

Confirm `spawn_tamed` sets `Tamed::owner` to the player entity — the payout
is credited via `inventories.get_mut(tamed.owner)`, so a different owner
makes every yield assertion read zero. `StructureTier` only exists from
Task 7 onward; until then, drop the `tier` parameter and its two lines.

---

### Task 1: `Biome::Platform` variant

**Files:**
- Modify: `crates/engine/src/world.rs:9-23` (the `Biome` enum)
- Modify: `crates/engine/src/save.rs:168` (`SAVE_FORMAT_VERSION`)
- Modify: `crates/tui/src/ui.rs:305-312` (`tile_style`)
- Modify: `crates/gui/src/render.rs:322-328` (the biome match)
- Modify: `assets/species/README.md`
- Test: `crates/engine/src/world.rs` (tests module), `crates/engine/src/species.rs` (tests module)

**Interfaces:**
- Consumes: nothing.
- Produces: `Biome::Platform` — a walkable biome that `WorldMap::classify` never returns and no shipped species lists as a habitat. Tasks 2–5 rely on it existing.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/world.rs`, inside `mod tests`:

```rust
#[test]
fn classify_never_produces_the_platform_biome() {
    let mut map = WorldMap::new(4242);
    for x in -60..60 {
        for y in -60..60 {
            assert_ne!(
                map.tile(x, y).biome,
                Biome::Platform,
                "Platform is stamped by a Home, never generated at ({x}, {y})"
            );
        }
    }
}
```

In `crates/engine/src/species.rs`, inside `mod tests`:

```rust
#[test]
fn no_shipped_species_lives_on_the_platform_biome() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species");
    let (db, _) = SpeciesDb::load_dir(&dir).unwrap();
    assert!(
        db.habitat_matches(Biome::Platform).is_empty(),
        "a base platform must have no ordinary habitat species — that's what makes it safe"
    );
    assert!(
        db.boss_habitat_matches(Biome::Platform).is_empty(),
        "a base platform must have no boss species either"
    );
}
```

`species.rs` already imports `crate::components::GlyphColor` and uses `Biome` in `SpeciesDef::habitats`; confirm `Biome` is in scope in the test module and add `use crate::world::Biome;` if not.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine platform_biome no_shipped_species_lives_on_the_platform`

Expected: FAIL to compile — `no variant named 'Platform' found for enum 'Biome'`.

- [ ] **Step 3: Add the variant**

In `crates/engine/src/world.rs`, add to `Biome`:

```rust
    BlackIce,
    /// The floor of a player base, stamped across the build radius when a
    /// Home is deployed (`Game::stamp_platform`) and never produced by
    /// `classify`. No shipped species lists it as a habitat, which is the
    /// entire mechanism behind a base being a safe haven —
    /// `Game::try_spawn_habitat_creature` bails when both candidate pools
    /// come back empty, so no spawn-suppression code is needed anywhere.
    Platform,
```

- [ ] **Step 4: Give both renderers a glyph**

`crates/tui/src/ui.rs`, in `tile_style`:

```rust
        Biome::StaticField => ('%', Color::White),
        Biome::Platform => ('_', Color::DarkGray),
```

`crates/gui/src/render.rs`, in the matching function:

```rust
        Biome::StaticField => ('%', WHITE),
        Biome::Platform => ('_', DARKGRAY),
```

`render.rs` imports `macroquad::prelude::*` (line 7), so `DARKGRAY` is already in scope.

- [ ] **Step 5: Bump the save format version**

`crates/engine/src/save.rs`:

```rust
pub const SAVE_FORMAT_VERSION: u32 = 9;
```

An enum gaining a variant is a `SaveData` shape change; bincode is positional, so v8 saves must be rejected rather than silently misdecoded. This single bump covers every save change in Tasks 1–7.

- [ ] **Step 6: Document the biome for modders**

Append to the habitats section of `assets/species/README.md`:

```markdown
`Platform` is the floor of a player's base. Nothing generated by the world
uses it — it only appears where a Home has been deployed. No shipped species
lists it, which is what makes a base free of wild spawns. Listing it in a
custom species' `habitats` is allowed and will make that species spawn inside
player bases; do it deliberately.
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS. Both renderers compile because their `Biome` matches are exhaustive with no wildcard arm — if either fails to compile, the new arm is missing.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/world.rs crates/engine/src/save.rs crates/engine/src/species.rs crates/tui/src/ui.rs crates/gui/src/render.rs assets/species/README.md
git commit -m "$(cat <<'EOF'
feat: add Biome::Platform for player base floors

Stamped only where a Home is deployed, never generated. No species lists it
as a habitat, so bases get wild-spawn immunity for free through the existing
empty-candidate-pool bail in try_spawn_habitat_creature.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `Platform` resource and slab stamping

**Files:**
- Modify: `crates/engine/src/resources.rs` (new `Platform` resource)
- Modify: `crates/engine/src/world.rs` (new `WorldMap::clear_override`)
- Modify: `crates/engine/src/lib.rs:2000-2092` (`place_structure`), `:2102+` (`remove_structure`), `:3283-3311` (`attack_nest`), `:564` and `:647` (resource insertion), `:789-837` (load path)
- Test: `crates/engine/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: `Biome::Platform` from Task 1.
- Produces:
  - `resources::Platform { pub center: Option<(i32, i32)> }` — a `Resource`.
  - `Game::stamp_platform(&mut self, cx: i32, cy: i32)` — private.
  - `Game::clear_platform(&mut self)` — private.
  - `Game::despawn_nest(&mut self, nest: Entity)` — private; untethers guardians then despawns.
  - Tasks 3 and 4 both depend on `Platform::center` being accurate.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`, inside `mod tests`. Note `place_home(game, dx, dy)` already exists at lib.rs:5779 and grants the 5 Core Fragments itself.

```rust
#[test]
fn placing_a_home_stamps_a_walkable_platform_across_the_build_radius() {
    let mut game = Game::new(920, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 1);
    let (hx, hy) = (ppos.x, ppos.y + 1);

    let mut map = game.world.resource_mut::<WorldMap>();
    for (dx, dy) in [
        (0, 0),
        (MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
        (-MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
    ] {
        let tile = map.tile(hx + dx, hy + dy);
        assert_eq!(
            tile.biome,
            Biome::Platform,
            "({dx}, {dy}) from Home should be platform floor"
        );
        assert!(tile.walkable, "platform floor must always be walkable");
    }
    assert_ne!(
        map.tile(hx + MAX_BUILD_DISTANCE_FROM_HOME + 1, hy).biome,
        Biome::Platform,
        "one tile past the build radius should still be natural terrain"
    );
}

#[test]
fn placing_a_home_obliterates_hostiles_and_nests_inside_the_radius_only() {
    let mut game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let inside = game.world.spawn((Hostile, Position { x: ppos.x + 3, y: ppos.y + 3 })).id();
    let outside = game
        .world
        .spawn((Hostile, Position { x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 2, y: ppos.y }))
        .id();
    let nest_inside = game
        .world
        .spawn((
            Nest { species: "sprite".to_string(), pending_respawns: Vec::new() },
            Position { x: ppos.x - 2, y: ppos.y + 1 },
        ))
        .id();

    place_home(&mut game, 0, 0);

    assert!(game.world.get_entity(inside).is_err(), "a hostile inside the radius is obliterated");
    assert!(game.world.get_entity(nest_inside).is_err(), "a nest inside the radius is obliterated");
    assert!(game.world.get_entity(outside).is_ok(), "a hostile outside the radius survives");
}

#[test]
fn obliterating_a_nest_untethers_a_guardian_standing_outside_the_radius() {
    let mut game = Game::new(922, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let nest = game
        .world
        .spawn((
            Nest { species: "sprite".to_string(), pending_respawns: Vec::new() },
            Position { x: ppos.x + 1, y: ppos.y },
        ))
        .id();
    let guardian = game
        .world
        .spawn((
            NestGuardian { nest },
            Position { x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 3, y: ppos.y },
        ))
        .id();

    place_home(&mut game, 0, 0);

    assert!(
        game.world.get::<NestGuardian>(guardian).is_none(),
        "a guardian outside the slab must lose its tether when its nest is obliterated, \
         not keep pointing at a despawned entity"
    );
}

#[test]
fn demolishing_the_home_clears_the_platform_back_to_natural_terrain() {
    let mut game = Game::new(923, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 1);

    let home = find_structure_by_kind(&mut game, "home").expect("the Home should be deployed");
    game.remove_structure(home).unwrap();

    assert_ne!(
        game.world.resource_mut::<WorldMap>().tile(ppos.x, ppos.y + 1).biome,
        Biome::Platform,
        "demolishing the Home should leave no orphan sanctuary behind"
    );
    assert!(
        game.world.resource::<Platform>().center.is_none(),
        "the platform resource should forget its center once the Home is gone"
    );
}

#[test]
fn no_wild_creature_ever_spawns_on_platform_floor() {
    let mut game = Game::new(924, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 0);

    for _ in 0..400 {
        game.try_spawn_habitat_creature(ppos.x + 2, ppos.y + 2);
    }
    let spawned = game
        .world
        .query_filtered::<Entity, With<Hostile>>()
        .iter(&game.world)
        .count();
    assert_eq!(spawned, 0, "platform floor has no habitat species, so nothing can spawn on it");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine platform`

Expected: FAIL to compile — `Platform` resource does not exist, `stamp_platform` does not exist.

- [ ] **Step 3: Add the `Platform` resource**

In `crates/engine/src/resources.rs`:

```rust
/// Center of the player's base platform — the slab of `Biome::Platform`
/// stamped across `MAX_BUILD_DISTANCE_FROM_HOME` when a Home is deployed.
/// `None` until the run's first Home goes down, which is why the opening
/// minutes of a run scale danger exactly as they did before platforms
/// existed.
///
/// Deliberately not serialized: it's reconstructed on load from the Home's
/// own position, which `SaveData::structures` already carries.
#[derive(Resource, Default, Clone, Copy)]
pub struct Platform {
    pub center: Option<(i32, i32)>,
}
```

- [ ] **Step 4: Add `WorldMap::clear_override`**

In `crates/engine/src/world.rs`, next to `set_override`:

```rust
    pub fn clear_override(&mut self, x: i32, y: i32) {
        self.overrides.remove(&(x, y));
    }
```

- [ ] **Step 5: Extract `despawn_nest` and reuse it in `attack_nest`**

`attack_nest` (lib.rs:3292-3305) already untethers guardians before despawning. Task 2 needs the same behaviour from a second call site, so extract it:

```rust
    /// Despawns `nest`, first stripping `NestGuardian` from every creature
    /// tethered to it so none is left pointing at a dead entity — they
    /// resume ordinary wandering. Despawning implicitly cancels anything
    /// left in `Nest::pending_respawns`.
    fn despawn_nest(&mut self, nest: Entity) {
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
    }
```

Then replace the inline block in `attack_nest` with `self.despawn_nest(nest);`.

- [ ] **Step 6: Implement stamping and clearing**

Add to `impl Game` in `crates/engine/src/lib.rs`:

```rust
    /// Stamps the base platform centered on `(cx, cy)`: every tile within
    /// `MAX_BUILD_DISTANCE_FROM_HOME` (Chebyshev) becomes walkable
    /// `Biome::Platform`, and every hostile and nest standing inside is
    /// obliterated. Deploying a Home and breaching into a new zone are the
    /// only callers.
    fn stamp_platform(&mut self, cx: i32, cy: i32) {
        {
            let mut map = self.world.resource_mut::<WorldMap>();
            for dy in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                for dx in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                    map.set_override(
                        cx + dx,
                        cy + dy,
                        Tile {
                            biome: Biome::Platform,
                            walkable: true,
                        },
                    );
                }
            }
        }

        let inside = |p: &Position| {
            (p.x - cx).abs() <= MAX_BUILD_DISTANCE_FROM_HOME
                && (p.y - cy).abs() <= MAX_BUILD_DISTANCE_FROM_HOME
        };
        let hostiles: Vec<Entity> = {
            let mut query = self.world.query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| inside(p))
                .map(|(e, _)| e)
                .collect()
        };
        for e in hostiles {
            self.world.despawn(e);
        }
        // Nests go through despawn_nest rather than a bare despawn: a
        // guardian can be standing outside the slab while its nest is
        // inside it, and would otherwise be left tethered to a dead entity.
        let nests: Vec<Entity> = {
            let mut query = self.world.query_filtered::<(Entity, &Position), With<Nest>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| inside(p))
                .map(|(e, _)| e)
                .collect()
        };
        for nest in nests {
            self.despawn_nest(nest);
        }

        self.world.resource_mut::<Platform>().center = Some((cx, cy));
    }

    /// Removes the platform slab, restoring natural terrain underneath.
    /// Called when the Home is demolished — the slab is defined as
    /// "centered on the current Home", so no Home means no slab.
    fn clear_platform(&mut self) {
        let Some((cx, cy)) = self.world.resource::<Platform>().center else {
            return;
        };
        {
            let mut map = self.world.resource_mut::<WorldMap>();
            for dy in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                for dx in -MAX_BUILD_DISTANCE_FROM_HOME..=MAX_BUILD_DISTANCE_FROM_HOME {
                    map.clear_override(cx + dx, cy + dy);
                }
            }
        }
        self.world.resource_mut::<Platform>().center = None;
    }
```

Ensure `Tile` and `Biome` are imported from `crate::world` at the top of `lib.rs`.

- [ ] **Step 7: Hook into placement and removal**

In `place_structure`, immediately before `self.log(format!("You deploy a {}.", def.name));` (lib.rs:2089):

```rust
        if def.id == HOME_STRUCTURE_ID {
            self.stamp_platform(x, y);
        }
```

In `remove_structure`, in the Home branch that cascades to every other structure, add `self.clear_platform();` alongside the cascade. Read the existing branch first and place the call so it runs exactly once, on Home removal only.

- [ ] **Step 8: Register the resource in both constructors**

Next to `world.insert_resource(ZoneLevel::default());` (lib.rs:564) in `Game::new`:

```rust
        world.insert_resource(Platform::default());
```

And in the load path (lib.rs:647 region), insert `Platform::default()` too. Then, after the structure-spawning loop finishes (after lib.rs:837), reconstruct the center from the restored Home:

```rust
        // The slab's tiles come back via SaveData::tile_overrides; only the
        // center needs rediscovering, and the Home's position is it.
        if let Some(home) = game.home_position() {
            game.world.resource_mut::<Platform>().center = Some((home.x, home.y));
        }
```

- [ ] **Step 9: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS, including all five new tests.

- [ ] **Step 10: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/resources.rs crates/engine/src/world.rs crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: deploying a Home stamps a base platform slab

The build radius becomes a physical 31x31 slab of Biome::Platform, walkable
and free of wild spawns. Placing a Home obliterates hostiles and nests inside
it; demolishing the Home clears it. Nests go through a new despawn_nest helper
so guardians outside the slab aren't left tethered to a dead entity.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Danger scaling measured from the platform edge

**Files:**
- Modify: `crates/engine/src/lib.rs:3543-3548` (`distance_stat_multiplier`), `:3579-3586` (`max_pack_size`)
- Test: `crates/engine/src/lib.rs:11498` and `:11532` (revise both existing tests), plus new cases

**Interfaces:**
- Consumes: `resources::Platform` from Task 2.
- Produces: `Game::distance_from_danger_origin(&self, x: i32, y: i32) -> i32` — private; Chebyshev distance from `ZoneSpawnPoint`, less the platform radius when a platform exists, clamped at 0.

- [ ] **Step 1: Revise the two existing tests and add the new cases**

`distance_stat_multiplier_grows_with_distance_from_the_zone_spawn_point_and_caps` (lib.rs:11498) and `max_pack_size_grows_with_zone_and_distance_and_caps_per_zone` (lib.rs:11532) currently assert the old origin. They must be **revised, not deleted** — they are still the regression check for the curve, only its origin moves.

Both existing tests run without a Home, so their current assertions stay valid as the "no platform" case. Rename them to say so and add the platform cases:

```rust
#[test]
fn distance_stat_multiplier_measures_from_the_zone_spawn_point_when_no_home_exists() {
    // Body unchanged from the current test at lib.rs:11498 — before the
    // run's first Home, scaling is exactly what it always was.
}

#[test]
fn distance_stat_multiplier_treats_the_whole_platform_as_distance_zero() {
    let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    place_home(&mut game, 0, 0);

    assert_eq!(
        game.distance_stat_multiplier(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME,
            spawn.y
        ),
        1.0,
        "the platform edge is still perfectly safe territory"
    );
    assert_eq!(
        game.distance_stat_multiplier(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES - 1,
            spawn.y
        ),
        1.0,
        "one tile short of the first step past the edge is still 1.0"
    );
    assert!(
        (game.distance_stat_multiplier(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES,
            spawn.y
        ) - 1.25)
            .abs()
            < f32::EPSILON,
        "the first step up lands one full step past the platform edge — 30 tiles from Home"
    );
    assert_eq!(
        game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
        MAX_DISTANCE_STAT_MULTIPLIER,
        "the cap is unchanged"
    );
}

#[test]
fn max_pack_size_also_counts_from_the_platform_edge() {
    let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    place_home(&mut game, 0, 0);

    assert_eq!(
        game.max_pack_size(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y),
        1,
        "packs don't grow inside territory that's still stat-x1.0"
    );
    assert_eq!(
        game.max_pack_size(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + PACK_SIZE_STEP_TILES,
            spawn.y
        ),
        2,
        "the first pack-size step lands one full step past the platform edge"
    );
}
```

Note `place_home` places relative to the *player*, and the player starts at `ZoneSpawnPoint`, so `place_home(&mut game, 0, 0)` centers the platform on the spawn point. If `place_home(0, 0)` fails because the player occupies that tile, use `(0, 1)` and offset the expected coordinates by the same amount.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine platform_as_distance_zero platform_edge`

Expected: FAIL — the first step still lands at `DISTANCE_STAT_STEP_TILES` from spawn, so the platform-edge assertion reports 1.25 where 1.0 is expected.

- [ ] **Step 3: Implement the shared origin helper**

Add to `impl Game`:

```rust
    /// Chebyshev distance from `(x, y)` to the edge of safe territory — the
    /// platform's edge once a Home exists, the bare `ZoneSpawnPoint`
    /// before then. Both danger curves measure from this rather than from
    /// the spawn point directly, so that the whole base counts as distance
    /// zero instead of sitting exactly on the first escalation step.
    fn distance_from_danger_origin(&self, x: i32, y: i32) -> i32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let dist = (x - spawn.x).abs().max((y - spawn.y).abs());
        if self.world.resource::<Platform>().center.is_some() {
            (dist - MAX_BUILD_DISTANCE_FROM_HOME).max(0)
        } else {
            dist
        }
    }
```

- [ ] **Step 4: Use it in both curves**

`distance_stat_multiplier` becomes:

```rust
    fn distance_stat_multiplier(&self, x: i32, y: i32) -> f32 {
        let dist = self.distance_from_danger_origin(x, y);
        let mult = 1.0 + (dist / DISTANCE_STAT_STEP_TILES) as f32 * DISTANCE_STAT_STEP_BONUS;
        mult.min(MAX_DISTANCE_STAT_MULTIPLIER)
    }
```

`max_pack_size` becomes:

```rust
    fn max_pack_size(&self, x: i32, y: i32) -> u32 {
        let zone = self.world.resource::<ZoneLevel>().0;
        let cap = zone + 1;
        let dist = self.distance_from_danger_origin(x, y);
        let grown = 1 + (dist / PACK_SIZE_STEP_TILES) as u32;
        grown.min(cap)
    }
```

Both keep their `&self` receiver — this is exactly why the platform center lives in a resource rather than being looked up with `home_position`, which needs `&mut self`.

Update the doc comments on `DISTANCE_STAT_STEP_TILES` (lib.rs:57) and `PACK_SIZE_STEP_TILES` (lib.rs:75) to say the step is counted from the platform edge when one exists.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: measure danger scaling from the platform edge

The build radius equals the first distance step, so the base previously sat
exactly on the boundary of escalating territory. Both distance_stat_multiplier
and max_pack_size now subtract the platform radius, putting the first step 30
tiles from Home. Behaviour before the first Home is unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Structures survive the breach

**Files:**
- Modify: `crates/engine/src/lib.rs:3377-3430` (`enter_next_zone`)
- Test: `crates/engine/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: `Game::stamp_platform` from Task 2.
- Produces: `enter_next_zone` repositions rather than despawns structures. Task 5 depends on the Portal being despawned *before* this runs.

- [ ] **Step 1: Write the failing tests**

```rust
/// Deploys a Home plus a Mining Node beside it and breaches, returning the
/// two entities so a caller can assert on what survived.
fn breach_with_a_base(game: &mut Game) -> (Entity, Entity) {
    place_home(game, 0, 1);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 1).unwrap();

    let mut query = game.world.query::<(Entity, &Structure)>();
    let mut home = None;
    let mut node = None;
    for (e, s) in query.iter(&game.world) {
        if s.kind == "home" {
            home = Some(e);
        } else if s.kind == "mining_node" {
            node = Some(e);
        }
    }
    (home.unwrap(), node.unwrap())
}

#[test]
fn breaching_carries_every_structure_and_its_offset_from_home() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, node) = breach_with_a_base(&mut game);
    let before = {
        let h = *game.world.get::<Position>(home).unwrap();
        let n = *game.world.get::<Position>(node).unwrap();
        (n.x - h.x, n.y - h.y)
    };

    game.enter_next_zone();

    assert!(game.world.get_entity(home).is_ok(), "the Home travels through the breach");
    assert!(game.world.get_entity(node).is_ok(), "so does everything built around it");
    let h = *game.world.get::<Position>(home).unwrap();
    let n = *game.world.get::<Position>(node).unwrap();
    assert_eq!(
        (n.x - h.x, n.y - h.y),
        before,
        "the base's layout must be preserved exactly, not reshuffled"
    );
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_eq!((h.x, h.y), (spawn.x, spawn.y), "the Home lands at the new spawn point");
}

#[test]
fn breaching_preserves_structure_durability_and_node_stock() {
    let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = breach_with_a_base(&mut game);
    game.world.get_mut::<Durability>(node).unwrap().hp = 7;
    game.world.get_mut::<ResourceNode>(node).unwrap().amount = 2;

    game.enter_next_zone();

    assert_eq!(game.world.get::<Durability>(node).unwrap().hp, 7, "damage travels with the structure");
    assert_eq!(game.world.get::<ResourceNode>(node).unwrap().amount, 2, "so does mined-down stock");
}

#[test]
fn breaching_restamps_the_platform_around_the_new_spawn_point() {
    let mut game = Game::new(942, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    breach_with_a_base(&mut game);

    game.enter_next_zone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_eq!(
        game.world.resource_mut::<WorldMap>().tile(spawn.x, spawn.y).biome,
        Biome::Platform,
        "the slab is re-stamped on the new map"
    );
    assert_eq!(
        game.world.resource::<Platform>().center,
        Some((spawn.x, spawn.y)),
        "and the resource follows it"
    );
}

#[test]
fn breaching_leaves_a_cronjob_assignment_pointing_at_a_live_structure() {
    let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = breach_with_a_base(&mut game);
    let worker = game
        .world
        .spawn((
            Tamed { owner: game.player_entity() },
            Task { kind: TaskKind::GatherResource, target: node, progress: 0, required: 10 },
        ))
        .id();

    game.enter_next_zone();

    let task = game.world.get::<Task>(worker).expect("the cronjob survives the breach");
    assert_eq!(task.target, node, "and still points at the structure that travelled with it");
    assert!(game.world.get_entity(task.target).is_ok(), "which is still alive");
}
```

The `Tamed`/`Task` constructor fields must match `components.rs` exactly — read them before writing this test and adjust the literals rather than guessing.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine breaching_`

Expected: FAIL — `get_entity(home).is_ok()` is false, because `enter_next_zone` currently despawns every `Structure`.

- [ ] **Step 3: Rewrite the despawn and add repositioning**

In `enter_next_zone`, replace the despawn block and the dangling-task block (lib.rs:3378-3396) with:

```rust
        let stale: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Hostile>, With<Nest>)>>();
            query.iter(&self.world).collect()
        };
        for e in stale {
            self.world.despawn(e);
        }

        // Snapshot every structure's offset from the Home before the map is
        // swapped, so the base can be rebuilt in the same layout around the
        // new spawn point. A Portal can't be built without a Home, and
        // demolishing a Home cascades to the Portal, so a Home is
        // guaranteed to exist by the time a breach happens.
        let home = self
            .home_position()
            .expect("breaching requires a Portal, which requires a Home");
        let offsets: Vec<(Entity, (i32, i32))> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Structure>>();
            query
                .iter(&self.world)
                .map(|(e, p)| (e, (p.x - home.x, p.y - home.y)))
                .collect()
        };
```

The dangling-`Task` cleanup is **deleted outright**, not commented out. It existed only because structures used to be despawned out from under their assigned workers; with structures surviving, a cronjob stays valid through the breach.

Then, after `self.world.insert_resource(ZoneSpawnPoint { ... })` (lib.rs:3411-3414) and before the `travelers` block:

```rust
        for (e, (dx, dy)) in offsets {
            if let Some(mut pos) = self.world.get_mut::<Position>(e) {
                pos.x = start.0 + dx;
                pos.y = start.1 + dy;
            }
        }
        // The new map is freshly generated, so its override overlay is
        // empty — the departed zone's slab went with the old WorldMap and
        // needs no cleanup. Only one slab ever exists at a time.
        self.stamp_platform(start.0, start.1);
```

Update `enter_next_zone`'s doc comment to describe carrying the base forward.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS. Existing tests asserting that structures vanish on breach (if any) will now fail — those assertions encoded the old write-off behaviour and must be revised to assert survival. Read each failure before changing it.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: the base travels through the breach

Structures are repositioned around the new spawn point at their original
offsets from Home instead of being despawned, and the platform slab is
re-stamped around them. Durability, node stock and Temporary counters travel
for free since the entities are never despawned.

Drops the dangling-Task cleanup: cronjob assignments now stay valid through a
breach because their target structures survive it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: One-use Portal

**Files:**
- Modify: `crates/engine/src/lib.rs:1173-1177` (`move_player`)
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: Task 4's carry-forward.
- Produces: nothing new; changes `move_player`'s portal branch.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn stepping_through_a_portal_consumes_it_so_it_never_travels() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
    game.place_structure("portal", 1, 0).unwrap();

    game.move_player(1, 0);

    assert_eq!(game.world.resource::<ZoneLevel>().0, 2, "stepping on the portal breaches");
    let portals = {
        let mut query = game.world.query::<&Structure>();
        query.iter(&game.world).filter(|s| s.kind == "portal").count()
    };
    assert_eq!(
        portals, 0,
        "a portal is one-use — carrying it forward would make every later breach free"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine stepping_through_a_portal_consumes_it`

Expected: FAIL — `portals` is 1, because Task 4 now carries every structure forward including the Portal.

- [ ] **Step 3: Consume the portal**

In `move_player`, replace lines 1173-1177:

```rust
        if let Some(portal) = self.find_zone_portal_at(nx, ny) {
            // Despawned before enter_next_zone snapshots the base, so it
            // isn't carried forward. Without this every breach after the
            // first would be free, since structures now survive a breach.
            self.world.despawn(portal);
            self.enter_next_zone();
            self.tick();
            return;
        }
```

- [ ] **Step 4: Document it**

In `assets/structures/README.md`, in the `zone_portal` field description, add:

```markdown
A `zone_portal` structure is consumed when the player steps onto it — it does
not travel to the next zone with the rest of the base. Each breach therefore
costs a fresh build.
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs assets/structures/README.md
git commit -m "$(cat <<'EOF'
feat: portals are consumed on use

Load-bearing now that structures survive a breach: a portal that travelled
with the base would make every breach after the first free, bypassing the
10-per-zone-level portal fragment cost entirely.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Zone-scaled node yields

**Files:**
- Modify: `crates/engine/src/systems.rs:130-199` (`task_progress_system`)
- Test: `crates/engine/src/systems.rs` or `crates/engine/src/lib.rs` (wherever cronjob yield tests already live — search for `task_progress` first)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: worked-node payout is `ZoneLevel::stat_multiplier()` units for unbanked resources, 1 for banked ones. Task 7 extends this to multiply by tier.

- [ ] **Step 1: Write the failing tests**

Uses `run_one_full_gather_cycle` from Shared Test Helpers (without the
`tier` parameter, which Task 7 adds).

```rust
#[test]
fn a_worked_node_pays_out_more_the_deeper_the_zone() {
    let mut game = Game::new(960, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    // stat_multiplier() for zone 4 is 1 << 3 == 8.
    assert_eq!(game.world.resource::<ZoneLevel>().stat_multiplier(), 8);

    let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

    assert_eq!(gained, 8, "a zone-4 node pays 8x what a zone-1 node pays");
}

#[test]
fn a_zone_one_node_still_pays_exactly_one() {
    let mut game = Game::new(962, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1, "runs start at zone 1");

    let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

    assert_eq!(
        gained, 1,
        "zone 1's multiplier is 1 << 0 == 1, so the opening game is unchanged"
    );
}

#[test]
fn a_banked_resource_never_scales_with_zone_depth() {
    let mut game = Game::new(961, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 5;

    let gained = run_one_full_gather_cycle(&mut game, ids::RESEARCH_DATA);

    assert_eq!(
        gained, 1,
        "research_data has a bank_limit of 200 — scaling it would fill the bank in ~13 cycles \
         and turn the research economy into 'no room to store it' spam"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine pays_out_more_the_deeper never_scales_with_zone`

Expected: FAIL — `gained` is 1 rather than 8, since the payout is hardcoded.

- [ ] **Step 3: Give the system the zone and scale the payout**

In `crates/engine/src/systems.rs`, add the resource parameter:

```rust
pub fn task_progress_system(
    mut tasks: Query<CronjobWorker>,
    mut nodes: Query<&mut ResourceNode>,
    mut inventories: Query<&mut Inventory>,
    species_db: Res<SpeciesDb>,
    item_db: Res<ItemDb>,
    zone: Res<ZoneLevel>,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
```

Import `ZoneLevel` from `crate::resources`.

Replace the payout block (systems.rs:160-170):

```rust
        node.amount -= 1;
        if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
            let def = item_db.get(node.resource.as_str());
            let resource_name = def.map(|d| d.name.as_str()).unwrap_or(node.resource.as_str());
            // A banked resource is excluded from zone scaling: its bank
            // limit is the pacing mechanism, and an exponential payout
            // would just overflow it every few cycles.
            let payout = if def.and_then(|d| d.bank_limit).is_some() {
                1
            } else {
                zone.stat_multiplier() as u32
            };
            let landed = inv.add_capped(node.resource.clone(), payout, &item_db);
            if landed == 0 {
                log.push(format!(
                    "A cronjob yields {resource_name} but there's no room to store it."
                ));
            }
```

Then change the success message to report the amount, keeping the existing `level_note`:

```rust
            log.push_kind(
                MessageKind::Loot,
                format!("Your subroutine extracted {landed} {resource_name}.{level_note}"),
            );
```

`resource_name` is now bound before the payout rather than inside the old block — check the surrounding borrow scopes compile and move the binding if `item_db` conflicts.

- [ ] **Step 4: Register the resource on the schedule if needed**

If `task_progress_system` is added to a bevy `Schedule` that runs against a `World` lacking `ZoneLevel`, the run will panic. `ZoneLevel` is inserted in `Game::new` (lib.rs:564) and the load path (lib.rs:647), so both real paths are covered — but confirm any test that runs the schedule against a hand-built `World` inserts it too.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS. Existing cronjob tests asserting exactly 1 unit per cycle will fail at zone 1 only if `stat_multiplier()` is misread — zone 1 is `1 << 0 == 1`, so they should be unaffected. Investigate any that do fail rather than adjusting the expected number reflexively.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/systems.rs crates/engine/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: worked node payouts scale with zone depth

Node output multiplies by ZoneLevel::stat_multiplier(), the same doubling base
as wild stats and GEAR_LEVEL_GROWTH. A flat base economy was the root cause of
settling never being worth the time. Banked resources are excluded — their
bank limit is the pacing mechanism.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Upgrade tiers

**Files:**
- Modify: `crates/engine/src/structures.rs` (new `UpgradeDef`, new `StructureDef::upgrade` field)
- Modify: `crates/engine/src/components.rs` (new `StructureTier` component)
- Modify: `crates/engine/src/lib.rs` (new `Game::upgrade_structure`, `EntityView::tier`, place/save/load paths)
- Modify: `crates/engine/src/systems.rs` (tier multiplies payout)
- Modify: `crates/engine/src/save.rs` (`StructureSave::tier`)
- Modify: `assets/structures/mining_node.ron`, `research_node.ron`, `compiler.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: the payout code from Task 6.
- Produces:
  - `structures::UpgradeDef { pub max_tier: u32, pub cost: Vec<(ItemId, u32)> }`
  - `StructureDef::upgrade: Option<UpgradeDef>` (`#[serde(default)]`)
  - `components::StructureTier(pub u32)`
  - `Game::upgrade_structure(&mut self, structure: Entity) -> Result<(), String>`
  - `EntityView::tier: Option<u32>` — Task 8 renders this.

- [ ] **Step 1: Write the failing tests**

Uses `count_item`, `find_structure_by_kind` and `run_one_full_gather_cycle`
from Shared Test Helpers. `run_one_full_gather_cycle` gains its `tier`
parameter in this task.

```rust
#[test]
fn upgrading_a_node_costs_materials_and_raises_its_tier() {
    let mut game = Game::new(970, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12 + 20);
    game.place_structure("mining_node", 1, 1).unwrap();
    let node = find_structure_by_kind(&mut game, "mining_node").unwrap();

    assert_eq!(
        game.world.get::<StructureTier>(node).unwrap().0,
        1,
        "structures deploy at Mk1"
    );
    let before = count_item(&game, ids::CORE_FRAGMENT);

    game.upgrade_structure(node).unwrap();

    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);
    assert_eq!(
        before - count_item(&game, ids::CORE_FRAGMENT),
        20,
        "reaching tier 2 costs the def's 10 per tier x 2"
    );
}

#[test]
fn upgrading_a_node_makes_its_extraction_more_reliable() {
    let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("mining_node", 1, 1).unwrap();
    let node = find_structure_by_kind(&mut game, "mining_node").unwrap();

    assert_eq!(game.world.get::<ResourceNode>(node).unwrap().level, Some(1));
    game.upgrade_structure(node).unwrap();
    assert_eq!(
        game.world.get::<ResourceNode>(node).unwrap().level,
        Some(2),
        "tier feeds ResourceNode.level, which already drives mining_success_chance"
    );
}

#[test]
fn upgrading_refuses_past_max_tier_and_without_materials() {
    let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 1).unwrap();
    let node = find_structure_by_kind(&mut game, "mining_node").unwrap();

    let err = game
        .upgrade_structure(node)
        .expect_err("no materials left after building it");
    assert!(err.contains("Not enough"), "unexpected error: {err}");

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);
    let max = game
        .world
        .resource::<StructureDb>()
        .get("mining_node")
        .unwrap()
        .upgrade
        .as_ref()
        .unwrap()
        .max_tier;
    for _ in 1..max {
        game.upgrade_structure(node).unwrap();
    }
    let err = game
        .upgrade_structure(node)
        .expect_err("a maxed node can't be upgraded further");
    assert!(err.contains("fully upgraded"), "unexpected error: {err}");
}

#[test]
fn a_structure_without_an_upgrade_def_cannot_be_upgraded() {
    let mut game = Game::new(973, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let home = find_structure_by_kind(&mut game, "home").unwrap();
    let err = game
        .upgrade_structure(home)
        .expect_err("Home declares no upgrade path");
    assert!(err.contains("can't be upgraded"), "unexpected error: {err}");
}

#[test]
fn tier_multiplies_payout_on_top_of_the_zone_multiplier() {
    let mut game = Game::new(974, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3; // stat_multiplier() == 4

    let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT, Some(3));

    assert_eq!(gained, 12, "tier 3 x zone multiplier 4");
}

#[test]
fn a_structures_tier_survives_a_save_and_load_round_trip() {
    let mut game = Game::new(975, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("mining_node", 1, 1).unwrap();
    let node = find_structure_by_kind(&mut game, "mining_node").unwrap();
    game.upgrade_structure(node).unwrap();
    game.upgrade_structure(node).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_tier_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = find_structure_by_kind(&mut loaded, "mining_node").unwrap();
    assert_eq!(
        loaded.world.get::<StructureTier>(restored).unwrap().0,
        3,
        "a Mk3 node must not come back as Mk1"
    );
    assert_eq!(
        loaded.world.get::<ResourceNode>(restored).unwrap().level,
        Some(3),
        "and its extraction reliability must be restored with it — WorkDef::level \
         only carries the tier-1 baseline"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine upgrading_ tier_multiplies`

Expected: FAIL to compile — `StructureTier` and `upgrade_structure` do not exist.

- [ ] **Step 3: Add the schema**

In `crates/engine/src/structures.rs`:

```rust
/// A structure's upgrade path — see `Game::upgrade_structure`. The cost to
/// reach tier N is each amount in `cost` multiplied by N, so upgrades get
/// steadily more expensive without needing a per-tier table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub max_tier: u32,
    pub cost: Vec<(ItemId, u32)>,
}
```

And on `StructureDef`:

```rust
    /// If set, this structure can be upgraded through tiers, each one
    /// multiplying its work payout and raising its `ResourceNode::level`
    /// (and so its `mining_success_chance`). `#[serde(default)]` so
    /// existing structure files (including mods) stay un-upgradeable
    /// exactly as before this field existed.
    #[serde(default)]
    pub upgrade: Option<UpgradeDef>,
```

In `crates/engine/src/components.rs`:

```rust
/// A structure's current upgrade tier, starting at 1. Present only on
/// structures whose definition sets `StructureDef::upgrade`.
#[derive(Component, Clone, Copy, Debug)]
pub struct StructureTier(pub u32);
```

- [ ] **Step 4: Attach the component on deploy and on load**

In `place_structure`, alongside the other conditional component inserts (lib.rs:2073-2088):

```rust
        if def.upgrade.is_some() {
            entity.insert(StructureTier(1));
        }
```

In the load path (lib.rs:795-836), after the `ResourceNode` insert:

```rust
            if def.upgrade.is_some() {
                entity.insert(StructureTier(s.tier.unwrap_or(1)));
            }
```

And restore the node level to match the tier, since `WorkDef::level` only carries the tier-1 baseline:

```rust
            if let Some(tier) = s.tier
                && let Some(mut node) = game.world.get_mut::<ResourceNode>(structure_id)
                && node.level.is_some()
            {
                node.level = Some(tier);
            }
```

- [ ] **Step 5: Persist it**

In `crates/engine/src/save.rs`:

```rust
pub struct StructureSave {
    pub kind: String,
    pub position: (i32, i32),
    pub resource_amount: Option<u32>,
    /// Current raid durability — see `components::Durability`.
    pub durability: Option<u32>,
    /// Current upgrade tier — see `components::StructureTier`. `None` for a
    /// structure whose def declares no upgrade path.
    pub tier: Option<u32>,
}
```

In the save-writing query (lib.rs:944-957), add `Option<&StructureTier>` to the tuple and `tier: tier.map(|t| t.0),` to the pushed record.

No further `SAVE_FORMAT_VERSION` bump — Task 1 already moved it to 9 and v9 has not shipped.

- [ ] **Step 6: Implement `upgrade_structure`**

```rust
    /// Advances `structure` one upgrade tier, charging its `UpgradeDef`
    /// cost scaled by the tier being reached. The new tier both multiplies
    /// the structure's work payout and becomes its `ResourceNode::level`,
    /// so extraction gets more reliable as well as more productive.
    pub fn upgrade_structure(&mut self, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(kind) = self.world.get::<Structure>(structure).map(|s| s.kind.clone()) else {
            return Err("That isn't a structure.".into());
        };
        let def = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        let Some(upgrade) = def.upgrade else {
            return Err(format!("{} can't be upgraded.", def.name));
        };
        let tier = self.world.get::<StructureTier>(structure).map(|t| t.0).unwrap_or(1);
        if tier >= upgrade.max_tier {
            return Err(format!("{} is already fully upgraded.", def.name));
        }
        let next = tier + 1;
        let cost: Vec<(ItemId, u32)> = upgrade
            .cost
            .iter()
            .map(|(item, qty)| (item.clone(), qty * next))
            .collect();

        let player = self.player_entity();
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty);
            }
        }

        self.world.entity_mut(structure).insert(StructureTier(next));
        // A node that opted into chance-based yield tracks its tier as its
        // level; one that always succeeds (level None) stays that way.
        if let Some(mut node) = self.world.get_mut::<ResourceNode>(structure)
            && node.level.is_some()
        {
            node.level = Some(next);
        }
        self.log(format!("You upgrade the {} to Mk{next}.", def.name));
        self.tick();
        Ok(())
    }
```

- [ ] **Step 7: Multiply the payout by tier**

In `crates/engine/src/systems.rs`, widen the node query and fold the tier in:

```rust
    mut nodes: Query<(&mut ResourceNode, Option<&StructureTier>)>,
```

Update the destructuring at the `nodes.get_mut(task.target)` site, then:

```rust
            let payout = if def.and_then(|d| d.bank_limit).is_some() {
                1
            } else {
                tier.map(|t| t.0).unwrap_or(1) * zone.stat_multiplier() as u32
            };
```

- [ ] **Step 8: Expose the tier on `EntityView`**

Add `pub tier: Option<u32>,` to `EntityView` (lib.rs:371) with a doc comment, and populate it in `view_entities` from the `StructureTier` component. Every other construction site of `EntityView` must set the new field — the compiler will list them.

- [ ] **Step 9: Give the three worked nodes an upgrade path**

`assets/structures/mining_node.ron`:

```ron
(
    id: "mining_node",
    name: "Mining Node",
    glyph: '$',
    color: Brown,
    build_cost: [("core_fragment", 12)],
    work: Some((produces: "core_fragment", ticks_per_unit: 10, level: Some(1))),
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 10)])),
)
```

`assets/structures/research_node.ron` — add:

```ron
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 10)])),
```

`assets/structures/compiler.ron` — add:

```ron
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 12)])),
```

- [ ] **Step 10: Document the schema**

Add an `upgrade` section to `assets/structures/README.md` describing `max_tier`, `cost`, the cost-times-target-tier rule, and that tier both multiplies work payout and becomes `ResourceNode.level` (so it raises extraction reliability, which saturates at level 6).

- [ ] **Step 11: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 12: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src crates/engine/src/save.rs assets/structures
git commit -m "$(cat <<'EOF'
feat: data-driven structure upgrade tiers

A new #[serde(default)] StructureDef::upgrade field gives worked nodes a tier
path. Tier both multiplies work payout and becomes ResourceNode.level, reusing
the existing mining_success_chance curve rather than adding new balance math.
This is the per-zone material sink that keeps settling an activity now that
the base travels intact.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Upgrade UI

**Files:**
- Modify: `crates/app-core/src/lib.rs:159-213` (`Mode`), `:550` (dispatch), `:705-724` (Playing keys), plus a new `handle_upgrade_key`
- Modify: `crates/tui/src/ui.rs:56-57` (mode grouping), `:157` (dispatch), plus a new `render_upgrade_menu`
- Modify: `crates/gui/src/render.rs:749` (dispatch), plus a new `draw_upgrade_menu`
- Test: `crates/app-core/src/lib.rs` (tests module)

**Interfaces:**
- Consumes: `Game::upgrade_structure` and `EntityView::tier` from Task 7.
- Produces: `Mode::Upgrade`, reachable with `U` from `Mode::Playing`.

- [ ] **Step 1: Write the failing test**

`test_app(seed)` (app-core lib.rs:1717) builds an `App` already in
`Mode::Playing`; `structure_count(&mut app)` already exists alongside it.

```rust
#[test]
fn pressing_u_opens_the_upgrade_picker_and_esc_closes_it() {
    let mut app = test_app(230);

    app.handle_key(GameKey::Char('U'));
    assert!(app.mode == Mode::Upgrade, "'U' should open the upgrade menu");

    app.handle_key(GameKey::Esc);
    assert!(app.mode == Mode::Playing, "Esc should return to play");
}

#[test]
fn the_upgrade_picker_skips_structures_with_no_upgrade_path() {
    let mut app = test_app(231);

    // Home is the first entry in the build menu, and declares no upgrade
    // path — same b/Enter/Up sequence the remove-flow test uses.
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Up);
    assert_eq!(structure_count(&mut app), 1, "Home should now be deployed");

    app.handle_key(GameKey::Char('U'));
    assert!(app.mode == Mode::Upgrade);
    app.handle_key(GameKey::Enter);
    assert!(
        app.mode == Mode::Upgrade,
        "with nothing upgradeable nearby the picker has no entry to select, so Enter \
         should leave the player in the menu rather than firing a doomed upgrade"
    );
}
```

Verify `selected_index(key, 0)` returns `None` for an empty list before
relying on the second assertion; if it returns `Some(0)` instead, the
handler needs an explicit empty-list guard and the test should assert that
guard instead.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-app-core upgrade_picker`

Expected: FAIL to compile — no `Mode::Upgrade` variant.

- [ ] **Step 3: Add the mode and its key**

In `Mode` (app-core lib.rs:159), after `RemoveConfirm`:

```rust
    /// Lists nearby structures with an upgrade path (see
    /// `Game::upgrade_structure`); picking one advances it a tier.
    Upgrade,
```

In the `Mode::Playing` key match (app-core lib.rs:713 region), next to the `R` binding:

```rust
            GameKey::Char('U') => {
                self.mode = Mode::Upgrade;
                return;
            }
```

`U` is the only free letter adjacent in meaning to `R` (Remove); `u` is already Symlink.

In the mode dispatch (app-core lib.rs:550):

```rust
            Mode::Upgrade => self.handle_upgrade_key(key),
```

- [ ] **Step 4: Implement the handler**

```rust
    fn handle_upgrade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure && e.tier.is_some())
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked = structures[idx].entity;
            let Some(game) = &mut self.game else { return };
            match game.upgrade_structure(picked) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }
```

Filtering on `tier.is_some()` keeps un-upgradeable structures out of the list rather than letting the player pick one and get an error.

- [ ] **Step 5: Render it in the TUI**

Add `Mode::Upgrade` to the popup-mode grouping at ui.rs:56, dispatch at ui.rs:157, and add:

```rust
fn render_upgrade_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure && e.tier.is_some())
        .collect();
    let mut lines = vec![Line::from(
        "Upgrade which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        lines.push(Line::from("(no upgradeable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        lines.push(menu_line(
            format!(
                "[{}] {} at ({}, {}) [Mk{}]",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                s.tier.unwrap_or(1),
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Upgrade Structure")),
        popup,
    );
}
```

`menu_line(text: String, selected: bool) -> Line<'static>` (ui.rs:215) and
the `Paragraph`/`Block` wrapper above are copied verbatim from
`render_remove_menu` (ui.rs:803-840), only the title and the per-row suffix
differ.

- [ ] **Step 6: Render it in the GUI**

Add the dispatch arm at render.rs:749 and a `draw_upgrade_menu` mirroring `draw_remove_menu` (render.rs:1037), using `text_row`/`item_row` and the same `[Mk{}]` suffix.

- [ ] **Step 7: Add it to the help screen**

Find where `R` is documented in the help text (search both renderers for `"Demolish"`) and add a matching `U — Upgrade a structure` line.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/app-core/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "$(cat <<'EOF'
feat: U opens a structure upgrade picker

Mirrors the existing R/remove flow across app-core and both renderers. Only
structures whose def declares an upgrade path are listed, so the picker can't
offer a choice that would just error.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Balance projection — settling beats rushing

**Files:**
- Modify: `crates/engine/src/balance.rs` (new projection + regression test)

**Interfaces:**
- Consumes: the zone multiplier and tier rules from Tasks 6 and 7.
- Produces: `balance::ticks_to_afford_portal(zone: u32, tier: u32, workers: u32) -> f64`.

- [ ] **Step 1: Write the failing test**

`balance.rs` is this repo's established home for pure, deterministic offline projections that run as fast regression tests. Add:

```rust
/// Ticks of a tiered, worked Mining Node needed to fund one Portal at
/// `zone`, routed through the Market's `portal_fragment` buy price.
///
/// Deliberately arithmetic rather than a live sim: this is the check that
/// the base economy keeps pace with the doubling curve, which is the entire
/// reason the travelling-base work happened. See
/// `docs/superpowers/specs/2026-07-24-travelling-base-design.md`.
pub fn ticks_to_afford_portal(
    zone: u32,
    tier: u32,
    workers: u32,
    ticks_per_unit: u32,
    portal_fragment_cost: u32,
    market_price: u32,
) -> f64 {
    let payout = (tier * ZoneLevel(zone).stat_multiplier() as u32) as f64;
    let success = (0.4 + tier as f64 * 0.1).min(1.0);
    let per_tick = payout * success / ticks_per_unit as f64 * workers as f64;
    let needed = (portal_fragment_cost * zone * market_price) as f64;
    needed / per_tick
}

#[test]
fn a_tiered_base_funds_deeper_portals_faster_than_shallow_ones() {
    // Market buys portal_fragment at 8 core fragments; a Portal costs
    // 10 fragments per zone level; a Mining Node cycles every 10 ticks.
    let shallow = ticks_to_afford_portal(1, 1, 1, 10, 10, 8);
    let deep = ticks_to_afford_portal(4, 3, 1, 10, 10, 8);
    assert!(
        deep < shallow,
        "a tiered base at depth must out-earn its own rising portal cost, or settling \
         is still a losing move: zone 1 Mk1 = {shallow:.0} ticks, zone 4 Mk3 = {deep:.0}"
    );
}

#[test]
fn base_income_keeps_pace_with_the_doubling_curve_across_zones() {
    let mut previous = f64::MAX;
    for zone in 1..=6 {
        let ticks = ticks_to_afford_portal(zone, 3, 1, 10, 10, 8);
        assert!(
            ticks <= previous,
            "portal funding time must not blow up with depth — it did at zone {zone}"
        );
        previous = ticks;
    }
}
```

Verify the Market's buy price and the Mining Node's `ticks_per_unit` from the `.ron` files before trusting the literals above; if either changed, load them from `StructureDb` in the test the way `best_gear_stats` loads from `ItemDb` (balance.rs:234).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine funds_deeper_portals keeps_pace_with_the_doubling`

Expected: FAIL to compile — `ticks_to_afford_portal` does not exist.

- [ ] **Step 3: Implement the projection**

Add the function above to `balance.rs`, importing `ZoneLevel` (already imported at balance.rs:11).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --workspace`

Expected: PASS. If `a_tiered_base_funds_deeper_portals_faster_than_shallow_ones` fails, the economy does **not** actually reward settling — stop and report the numbers rather than adjusting the assertion to fit.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/balance.rs
git commit -m "$(cat <<'EOF'
test: prove a tiered base outpaces its own rising portal cost

Offline arithmetic projection in the same style as the existing zone-scaling
sweeps. This is the regression check that the flat-vs-exponential mismatch
stays fixed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Final verification

- [ ] `cargo test --workspace` — full suite green
- [ ] `cargo clippy --workspace` — no new warnings
- [ ] `cargo fmt --check` — clean
- [ ] `cargo run -p feral-processes` — deploy a Home, confirm the slab renders, build and upgrade a Mining Node, build a Portal, breach, confirm the base arrives intact and the Portal is gone

The last item is the one thing the tests can't cover: whether arriving inside your own sanctuary every breach feels right, or whether losing the "step into a dangerous unknown" moment costs more than the travelling base is worth. That is a judgement call for the user's eyes, flagged as an open risk in the spec.
