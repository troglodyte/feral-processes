//! Populating a zone with wild programs, nests, and habitat-born
//! creatures.

use crate::*;

impl Game {
    /// Spawns a wild creature of `species_id` at `(x, y)`, returning its
    /// `Entity` — `None` only if `species_id` isn't in `SpeciesDb` (every
    /// real call site passes an id it already validated against
    /// `SpeciesDb`, so this is a defensive no-op path, not an expected
    /// outcome). `spawn_nest_guardian` uses the returned entity to attach
    /// `NestGuardian`.
    pub(crate) fn spawn_wild_creature(
        &mut self,
        species_id: &str,
        x: i32,
        y: i32,
    ) -> Option<Entity> {
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(species_id)
            .cloned()?;
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

    /// Spawns a `Nest` for `species_id` at `(x, y)`, plus an initial
    /// `NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX` guardians clustered within
    /// `NEST_TETHER_RADIUS` of it.
    pub(crate) fn spawn_nest(&mut self, species_id: &str, x: i32, y: i32) {
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
    pub(crate) fn spawn_nest_guardian(
        &mut self,
        nest: Entity,
        species_id: &str,
        nest_x: i32,
        nest_y: i32,
    ) {
        let (gx, gy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (
                nest_x + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
                nest_y + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
            )
        };
        if let Some(guardian) = self.spawn_wild_creature(species_id, gx, gy) {
            self.world
                .entity_mut(guardian)
                .insert(NestGuardian { nest });
        }
    }

    /// Stat multiplier for a wild spawn at `(x, y)`, from how far it is
    /// (Chebyshev distance — matching 8-directional movement, so it's
    /// "how many moves away") from `ZoneSpawnPoint`: `1.0` right at spawn,
    /// growing by `DISTANCE_STAT_STEP_BONUS` every
    /// `DISTANCE_STAT_STEP_TILES`, capped at `MAX_DISTANCE_STAT_MULTIPLIER`.
    /// Applied multiplicatively with `ZoneLevel::stat_multiplier` in
    /// `spawn_wild_creature` — venturing away from where you breached in
    /// is its own escalating risk, independent of zone depth.
    pub(crate) fn distance_stat_multiplier(&self, x: i32, y: i32) -> f32 {
        let dist = self.distance_from_danger_origin(x, y);
        let mult = 1.0 + (dist / DISTANCE_STAT_STEP_TILES) as f32 * DISTANCE_STAT_STEP_BONUS;
        mult.min(MAX_DISTANCE_STAT_MULTIPLIER)
    }

    /// Chebyshev distance from `(x, y)` to the edge of safe territory: the
    /// platform's edge once a Home exists, the bare `ZoneSpawnPoint` before
    /// then. Both danger curves measure from this rather than straight from
    /// the spawn point, so the whole base counts as distance zero instead of
    /// sitting part-way up the first escalation step. The build radius (7)
    /// and `DISTANCE_STAT_STEP_TILES` (15) are independent dials: shrinking
    /// the platform pulls the first step inward, to 22 tiles from spawn.
    pub(crate) fn distance_from_danger_origin(&self, x: i32, y: i32) -> i32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let dist = (x - spawn.x).abs().max((y - spawn.y).abs());
        if self.world.resource::<Platform>().center.is_some() {
            (dist - MAX_BUILD_DISTANCE_FROM_HOME).max(0)
        } else {
            dist
        }
    }

    /// Rolls a fresh `Potential` for a newly created creature — see
    /// `spawn_wild_creature`/`fuse_companions`. Each of the four fields is
    /// independently uniform in `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// — the "same species, different stats" mechanic.
    pub(crate) fn roll_potential(&mut self) -> Potential {
        let mut rng = self.world.resource_mut::<GameRng>();
        Potential {
            hp_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            atk_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            def_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            growth_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
        }
    }

    /// Maximum wild pack size at `(x, y)`: capped at `zone + 1` (zone 1 →
    /// 2, zone 2 → 3, ...), reached gradually the farther `(x, y)` is from
    /// `ZoneSpawnPoint` — solo right at spawn, then one more potential
    /// packmate every `PACK_SIZE_STEP_TILES`. Used both to pick how many
    /// creatures a group spawn roll places together
    /// (`try_spawn_habitat_creature`) and as a hard ceiling on how many
    /// can ever end up in one fight (`gather_pack`).
    pub(crate) fn max_pack_size(&self, x: i32, y: i32) -> u32 {
        let zone = self.world.resource::<ZoneLevel>().0;
        let cap = (zone * PACK_SIZE_PER_ZONE).clamp(1, MAX_PACK_SIZE);
        let dist = self.distance_from_danger_origin(x, y);
        let grown = 1 + (dist / PACK_SIZE_STEP_TILES) as u32;
        grown.min(cap)
    }

    /// Spawns `count` wild creatures near the player, retrying with a fresh
    /// random offset whenever a roll whiffs (an unwalkable tile, or a biome
    /// with no matching species) rather than giving up on that slot — a
    /// freshly generated zone's terrain noise can otherwise leave large
    /// unwalkable or habitat-sparse patches right around the player's
    /// start point (see `find_walkable_start`, which always searches out
    /// from world origin), and a blind one-attempt-per-slot approach would
    /// leave the zone nearly empty whenever that happens. Bounded to
    /// `count * 20` attempts so a pathologically bad pocket can't loop
    /// forever instead of just spawning fewer than `count`.
    pub(crate) fn spawn_initial_creatures(&mut self, count: usize) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        // A base platform lists no habitat species, so every roll landing
        // inside one is a guaranteed miss. The platform is exactly as wide
        // as the default scatter and the player materializes at its center,
        // so without pushing the scatter out past its edge a zone breached
        // into with a base would be born completely empty.
        let reach = INITIAL_SPAWN_SCATTER_TILES
            + if self.world.resource::<Platform>().center.is_some() {
                MAX_BUILD_DISTANCE_FROM_HOME
            } else {
                0
            };
        let mut spawned = 0;
        let mut attempts = 0;
        while spawned < count && attempts < count * 20 {
            attempts += 1;
            let (dx, dy) = {
                let mut rng = self.world.resource_mut::<GameRng>();
                (
                    rng.0.random_range(-reach..=reach),
                    rng.0.random_range(-reach..=reach),
                )
            };
            if self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy) {
                spawned += 1;
            }
        }
    }

    pub(crate) fn maybe_spawn_wild_creature(&mut self) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        // Roll first: culling is wasted work if nothing was going to spawn.
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(0.05)
        };
        if !roll {
            return;
        }
        // At `WILD_CREATURE_CAP`, make room by despawning the `Hostile`
        // farthest (Chebyshev, matching 8-directional movement) from where
        // the player is now — the one least likely to ever be encountered
        // again. `NestGuardian`s are eligible like any other hostile; a cull
        // is a plain despawn, so it deliberately doesn't feed the nest's
        // `pending_respawns` the way an actual defeat does. Guardian counts
        // are best-effort once a nest is far behind the player.
        let hostiles: Vec<(Entity, i32)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&self.world)
                .map(|(e, p)| {
                    (
                        e,
                        (p.x - player_pos.x).abs().max((p.y - player_pos.y).abs()),
                    )
                })
                .collect()
        };
        if hostiles.len() >= WILD_CREATURE_CAP
            && let Some(&(farthest, _)) = hostiles.iter().max_by_key(|(_, dist)| *dist)
        {
            self.world.despawn(farthest);
        }
        let (dx, dy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (rng.0.random_range(-12..=12), rng.0.random_range(-12..=12))
        };
        self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy);
    }

    /// Attempts to spawn one habitat-appropriate wild creature (or, away
    /// from the zone's spawn point, a small pack of the same species — see
    /// `max_pack_size`) at `(x, y)`, returning whether it actually spawned
    /// anything — `false` on an unwalkable tile or a biome with no
    /// matching species, so callers (see `spawn_initial_creatures`) can
    /// retry elsewhere instead of silently losing that spawn slot.
    pub(crate) fn try_spawn_habitat_creature(&mut self, x: i32, y: i32) -> bool {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !tile.walkable {
            return false;
        }
        let species_db = self.world.resource::<SpeciesDb>();
        let candidates: Vec<String> = species_db
            .habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        let boss_candidates: Vec<String> = species_db
            .boss_habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if candidates.is_empty() && boss_candidates.is_empty() {
            return false;
        }
        // A boss takes the tile's one spawn slot instead of an ordinary
        // habitat creature, but only rarely, and only where one is defined
        // for this biome at all.
        let spawn_boss = !boss_candidates.is_empty() && {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(BOSS_SPAWN_CHANCE)
        };
        let pool = if spawn_boss || candidates.is_empty() {
            &boss_candidates
        } else {
            &candidates
        };
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
        for i in 0..group_size {
            // The first member anchors the roll's own tile; the rest
            // cluster loosely around it (walkability isn't rechecked for
            // these — same looseness the rest of spawning already has).
            let (gx, gy) = if i == 0 {
                (x, y)
            } else {
                let mut rng = self.world.resource_mut::<GameRng>();
                (
                    x + rng.0.random_range(-PACK_GATHER_RADIUS..=PACK_GATHER_RADIUS),
                    y + rng.0.random_range(-PACK_GATHER_RADIUS..=PACK_GATHER_RADIUS),
                )
            };
            self.spawn_wild_creature(&pick, gx, gy);
        }
        true
    }
}
