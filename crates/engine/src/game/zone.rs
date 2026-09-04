//! The zone the player currently stands in: locating things on the map,
//! and stepping through a portal to the next zone.

use crate::tuning::{
    NEST_CACHE_CREDIT_ZONE_BONUS, NEST_CACHE_CREDITS, NEST_CACHE_EQUIPMENT_ROLLS,
    NEST_CACHE_WORK_RESOURCE_MULT, NEST_ORPHAN_CHANCE, WORK_RESOURCE_DROP,
};
use crate::*;

impl Game {
    pub(crate) fn find_wild_creature_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), (With<Creature>, Without<Tamed>)>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Finds a `Nest` at `(x, y)`, if any — checked in `move_player`
    /// before the ordinary blocking-structure check, so walking into a
    /// nest tile attacks it instead of just being blocked.
    pub(crate) fn find_nest_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Nest>>();
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
    pub(crate) fn attack_nest(&mut self, nest: Entity) {
        // On every hit, not just the first: a guardian that wandered
        // outside its tether and walked home (see `wander_ai_system`)
        // would otherwise go unprovoked by the next swing.
        self.provoke_nest(nest);
        let player = self.player_entity();
        let label = self.entity_label(nest);
        // Deterministic, and shared with the base's rock — see
        // `Game::swing_damage` for why neither goes through
        // `battle::resolve_attack`.
        let dmg = self.swing_damage(player);
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
            // Reads Nest::species, so this has to run before despawn_nest
            // deletes the component it's reading.
            self.grant_nest_cache(nest);
            self.despawn_nest(nest);
        } else {
            self.log(format!(
                "You unleash a data strike into the {label} for {dmg} damage."
            ));
        }
    }

    /// Pays the loot a destroyed `nest` owes, drawn entirely from its
    /// species' existing `SpeciesDef` fields — content stays data, only the
    /// `NEST_CACHE_*` magnitudes in `tuning.rs` are code. Mirrors
    /// `award_loot`'s clone-then-drop-the-borrow shape (`combat_rewards.rs`)
    /// so the `SpeciesDb` resource isn't held live across a `grant_loot`
    /// call that itself needs the world.
    ///
    /// No XP: the guardians already paid that on the way down, and this is
    /// the structure itself coming down.
    pub(crate) fn grant_nest_cache(&mut self, nest: Entity) {
        let Some(species_id) = self.world.get::<Nest>(nest).map(|n| n.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        if let Some(resource) = &species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(WORK_RESOURCE_DROP) * NEST_CACHE_WORK_RESOURCE_MULT
            };
            let landed = self.grant_loot(resource.clone(), qty, LootSource::Cache);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!(
                        "The wreckage yields {} {}.",
                        landed,
                        self.item_name(resource)
                    ),
                );
            }
        }

        for _ in 0..NEST_CACHE_EQUIPMENT_ROLLS {
            for (item, chance) in self.equipment_drops_for(&species) {
                let roll = {
                    let mut rng = self.world.resource_mut::<GameRng>();
                    rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
                };
                if roll {
                    let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                    self.log_kind(
                        MessageKind::Loot,
                        format!("The wreckage also yields a {}!", self.drop_label(&copy)),
                    );
                }
            }
        }

        let zone_bonus = {
            let zone = self.world.resource::<ZoneLevel>().0;
            NEST_CACHE_CREDIT_ZONE_BONUS * zone.saturating_sub(1)
        };
        let qty = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(NEST_CACHE_CREDITS) + zone_bonus
        };
        let landed = self.grant_loot(self.trade_currency(), qty, LootSource::Cache);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("The cache holds {landed} credits!"),
            );
        }

        self.leave_nest_orphan(nest, &species_id);
    }

    /// Rolls `NEST_ORPHAN_CHANCE` for the thing a nest is actually cleared
    /// for: a program of the nest's **own** species, left running in the
    /// wreckage and joining the roster free.
    ///
    /// Its own species rather than a habitat draw so which nest the player
    /// walks up to is a real choice — you hunt the nest of the program you
    /// want. Free where `Game::adopt_orphan` charges a taming catalyst,
    /// because the Stack's orphan is an opportunity walked past and a nest
    /// is a fight already paid for.
    ///
    /// A full roster loses it, and says so. The alternative — refusing to
    /// destroy the nest at all — would make a structure's destruction
    /// conditional on unrelated state, and by the time this runs the caller
    /// has already committed to the `despawn_nest` on the next line.
    ///
    /// That ordering is also why the nest's `Position` is still readable
    /// here: it is the last thing to read anything off the entity, and it
    /// shares the reason `grant_nest_cache` itself runs before the despawn.
    fn leave_nest_orphan(&mut self, nest: Entity, species_id: &str) {
        let left_behind = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(NEST_ORPHAN_CHANCE)
        };
        if !left_behind {
            return;
        }
        let Some(at) = self.world.get::<Position>(nest).copied() else {
            return;
        };
        if self.pet_count() >= self.pet_capacity() {
            self.log_kind(
                MessageKind::Loot,
                "Something small was still running in the wreckage. You have no room for it.",
            );
            return;
        }
        // Scaled like any other spawn in this zone; the depth multiplier a
        // Stack orphan carries has no meaning on the surface.
        let Some(program) = self.adopt_program(species_id, at.x, at.y, 1.0) else {
            return;
        };
        let name = self.creature_label(program);
        self.log_kind(
            MessageKind::Loot,
            format!("{name} was still running in the wreckage. It comes with you."),
        );
    }

    /// Despawns `nest`, first stripping `NestGuardian` and `Pursuing` from
    /// every creature tethered to it so none is left pointing at a dead
    /// entity or chasing on its behalf — they resume ordinary wandering.
    /// Despawning implicitly cancels anything left in
    /// `Nest::pending_respawns`.
    pub(crate) fn despawn_nest(&mut self, nest: Entity) {
        let guardians: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&self.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        for guardian in guardians {
            self.world
                .entity_mut(guardian)
                .remove::<(NestGuardian, Pursuing)>();
        }
        self.world.despawn(nest);
    }

    /// Sets `Pursuing` on every living guardian tethered to `nest` — the
    /// whole effect of an attack landing. Collected in an inner scope
    /// first so the query's borrow of `self.world` ends before the
    /// `entity_mut` loop, the same shape `despawn_nest` above uses.
    pub(crate) fn provoke_nest(&mut self, nest: Entity) {
        let guardians: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&self.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        for guardian in guardians {
            self.world.entity_mut(guardian).insert(Pursuing);
        }
    }

    /// Whether `nest` currently has at least one living guardian marked
    /// `Pursuing` — `nest_respawn_tick` uses this so a replacement spawned
    /// while the nest is under siege arrives already provoked, instead of
    /// standing there calm until the player's next hit reaches it.
    pub(crate) fn nest_has_pursuers(&mut self, nest: Entity) -> bool {
        let mut query = self.world.query::<(&NestGuardian, Option<&Pursuing>)>();
        query
            .iter(&self.world)
            .any(|(g, pursuing)| g.nest == nest && pursuing.is_some())
    }

    /// Every tile a deployed structure stands on — the set a hauler's walk
    /// refuses, from the `Game` side. `haul_step_system` builds the same set
    /// from its own query; both go through `hauling::structure_tiles` so the
    /// two cannot disagree about what a blocked tile is.
    pub(crate) fn structure_tiles(&mut self) -> std::collections::HashSet<(i32, i32)> {
        let mut query = self.world.query_filtered::<&Position, With<Structure>>();
        let positions: Vec<Position> = query.iter(&self.world).copied().collect();
        crate::game::base::hauling::structure_tiles(positions.into_iter())
    }

    /// The `Structure` standing at `(x, y)`, if any — and `None` outright
    /// whenever the party is not in base space, regardless of what `(x, y)`
    /// numerically is.
    ///
    /// **`Structure` is the space tag** (see `docs/seams.md`): every
    /// structure stands in `base_grid::BaseGrid`'s coordinate space, never
    /// the zone surface, so a `Structure` query only ever answers a
    /// base-space question. Gating on `in_base` closes that generally
    /// instead of at each call site — `game/stack.rs`'s `link_site_free`
    /// used to call this with **surface** coordinates while scattering
    /// Stack entrances, and with the zone spawn point and base space's
    /// origin both commonly `(0, 0)`, it silently refused valid link sites
    /// near a base. The one legitimate caller left with `(x, y)` computed
    /// while off base (`place_structure`'s founding Home) is asking about a
    /// base that cannot exist yet — no Home means no other structure either,
    /// since removing a Home cascades to every structure it stands beside —
    /// so `None` is the right answer there too, not a special case.
    pub(crate) fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// The Home structure's position, if one is deployed anywhere right
    /// now — the anchor `place_structure` measures the build radius from.
    pub(crate) fn home_position(&mut self) -> Option<Position> {
        let mut query = self.world.query::<(&Structure, &Position)>();
        query
            .iter(&self.world)
            .find(|(s, _)| s.kind == HOME_STRUCTURE_ID)
            .map(|(_, p)| *p)
    }

    /// Finds a zone-portal structure (`StructureDef::zone_portal`) at
    /// `(x, y)`, if any — checked from `Game::move_in_base` so walking onto
    /// one breaches the zone. `(x, y)` is a base-space coordinate: a Portal
    /// is a `Structure`, and every `Structure` stands in base space now, so
    /// this refuses to answer at all outside it — see
    /// `find_blocking_structure_at`'s doc for why that guard belongs here
    /// rather than at each caller.
    pub(crate) fn find_zone_portal_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position, &Structure), ()>();
        let (entity, kind) = query
            .iter(&self.world)
            .find(|(_, p, _)| p.x == x && p.y == y)
            .map(|(e, _, s)| (e, s.kind.clone()))?;
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .is_some_and(|d| d.zone_portal)
            .then_some(entity)
    }

    /// Raises the world's tier.
    ///
    /// A breach used to be a migration: every hostile, nest and Stack
    /// entrance despawned, a fresh `WorldMap` carved from a fresh seed,
    /// the party and the base anchor teleported onto it, and a zone's
    /// worth of economy — the buyback shelves, the caravan, the spendable
    /// currencies — destroyed on the way through. Nothing of the sector
    /// you left survived, which is why nothing in it was ever worth
    /// knowing.
    ///
    /// The world is persistent now. There is one map for the run, minted
    /// at `Game::new`, and a breach raises the tier that everything
    /// spawned into it is scaled against. The party does not move; the
    /// ground under them does not change; what changes is what walks on
    /// it. That is the infrastructure settlements need — a place can only
    /// be worth returning to if it is still there.
    ///
    /// Two lines below look like the wipe code that was deleted around
    /// them and are the opposite — they are the mechanism:
    ///
    /// - Clearing `PopulatedChunks` is what makes the world visibly harden.
    ///   It marks which chunks have been stocked, so emptying it sends
    ///   `Game::ensure_local_population` back over ground it has already
    ///   covered to re-stock it at the new tier.
    /// - Clearing `StackMemory` is what makes an entrance re-tier. A
    ///   surviving link keys a `FrameSpec` that now folds in the tier, so
    ///   the frame behind it is re-carved and the memory of the old one —
    ///   which cells were seen, which caches were emptied, which lair was
    ///   cleared — describes a frame that no longer exists.
    pub(crate) fn enter_next_zone(&mut self) {
        self.notify(crate::notifications::NotificationKind::Breach);

        let new_level = {
            let mut zone = self.world.resource_mut::<ZoneLevel>();
            zone.0 += 1;
            zone.0
        };

        self.world.insert_resource(StackMemory::default());
        self.world
            .insert_resource(crate::resources::PopulatedChunks::default());

        self.log(format!(
            "You breach the portal and materialize in a level {new_level} sector. Hostile signal strength has spiked."
        ));

        self.ensure_local_population();
        self.ensure_local_settlements();
    }

    /// Breaches forward until the party is standing in `zone`, for the
    /// `savetool` binary — testing zone 6 otherwise means playing to zone 6.
    ///
    /// Deliberately a loop over the real `enter_next_zone` rather than a
    /// write to `ZoneLevel`: everything that makes a breach coherent — the
    /// two resets, and the ground re-stocking at each tier on the way past
    /// — lives in that function, and a shortcut would produce a save that no
    /// amount of play could have reached. That matters more now, not less:
    /// a written tier is a world that never hardened. `enter_next_zone` is
    /// `pub(crate)` and a `src/bin/` target is a separate crate, so this is
    /// also the seam that lets the tool reach it at all.
    ///
    /// Only runs forward: a breach consumes the portal and there is no way
    /// back, so a backwards warp is refused rather than silently ignored.
    pub fn warp_to_zone(&mut self, zone: u32) -> Result<(), String> {
        let current = self.world.resource::<ZoneLevel>().0;
        if zone <= current {
            return Err(format!(
                "already in zone {current}; a breach only runs forward, so zone {zone} is unreachable"
            ));
        }
        for _ in current..zone {
            self.enter_next_zone();
        }
        Ok(())
    }

    /// Where the player materialized on breaching into the current zone —
    /// see `resources::ZoneSpawnPoint`. Not drawn on the map: the outline
    /// that used to mark it was removed, and what the point still decides is
    /// the centre `in_opening_ring` and `frames_at` measure from.
    pub fn zone_spawn_point(&self) -> (i32, i32) {
        let p = self.world.resource::<ZoneSpawnPoint>();
        (p.x, p.y)
    }
}

/// The first walkable tile found spiralling out from the origin — where the
/// player is dropped when a zone is generated.
pub(crate) fn find_walkable_start(world_map: &mut WorldMap) -> (i32, i32) {
    for r in 0..64i32 {
        for dx in -r..=r {
            for dy in -r..=r {
                if r != 0 && dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if world_map.tile(dx, dy).walkable {
                    return (dx, dy);
                }
            }
        }
    }
    (0, 0)
}
