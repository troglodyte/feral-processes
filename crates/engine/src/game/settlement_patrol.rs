//! A Hostile town's patrol: who fields one, where its members stand, and
//! what makes them notice the party.
//!
//! `settlement_relations` holds what a town *thinks* of the party; this
//! holds the one consequence of that opinion the player meets on the ground
//! rather than on a screen. The tether itself is `components::TownPatrol`
//! and the chase is `Game::pursuit_tick` — shared with nest guardians and
//! written once. What is here is everything the two tethers do *not* share:
//! a spawner keyed to a band rather than to a nest, and a provocation that
//! is proximity rather than an attack.

use bevy_ecs::prelude::*;
use rand::RngExt;

use crate::Game;
use crate::components::{Position, TownPatrol};
use crate::game::spawning::ring_tiles;
use crate::resources::GameRng;
use crate::tuning::{
    SETTLEMENT_PATROL_AGGRO_RADIUS, SETTLEMENT_PATROL_KILL_STANDING, SETTLEMENT_PATROL_RANGE,
    SETTLEMENT_PATROL_RESPAWN_TICKS, SETTLEMENT_PATROL_RING_MAX, SETTLEMENT_PATROL_RING_MIN,
    SETTLEMENT_PATROL_SIZE,
};
use crate::world::WorldMap;

impl Game {
    /// Rolls for a Hostile neighbour to field one more patrol member.
    ///
    /// **Roll first, gate after**, `maybe_spawn_wild_creature`'s discipline:
    /// a miss spends exactly one draw and touches nothing else, so the
    /// seeded spawn tests move by a fixed amount rather than by whatever the
    /// world happens to hold.
    ///
    /// The chance is the reciprocal of `SETTLEMENT_PATROL_RESPAWN_TICKS`, so
    /// that constant reads as the *mean* wait for one member rather than an
    /// exact countdown. A nest keeps its countdown on the `Nest` entity;
    /// a town has nowhere to keep one — `Settlements` is the record that
    /// survives a save and a timer in it would be a save field for a figure
    /// nobody can see.
    pub(crate) fn maybe_field_patrol(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool(1.0 / SETTLEMENT_PATROL_RESPAWN_TICKS as f64)
        };
        if !roll {
            return;
        }
        self.field_patrol();
    }

    /// Everything fielding a patrol member *is*, once the roll has decided
    /// one happens — `run_raid`'s split from `raid_check`, and for its
    /// reason: the gates belong with the act, so a test that drives this
    /// directly is still testing the gates a player meets.
    pub(crate) fn field_patrol(&mut self) -> Option<Entity> {
        // The surface in *either* direction, `pursuit_tick`'s guard: the
        // player's `Position` is pinned to the anchor in base space and to
        // the entrance tile in the Stack, so a range measured from it would
        // field patrols around a party that is nowhere near them.
        if self.is_underground() || self.in_base() {
            return None;
        }
        let candidates = self.towns_short_of_a_patrol();
        if candidates.is_empty() {
            return None;
        }
        // One draw, over `Settlements`' own `BTreeMap` order —
        // `town_raid_check`'s rule, and the same reason: any other order is
        // seed-unstable across a save round trip.
        let (town, tile) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            candidates[rng.0.random_range(0..candidates.len())]
        };
        self.spawn_patrol_member(town, tile)
    }

    /// Every known town near enough to the *party* to bother fielding a
    /// patrol, angry enough to want one, and not already at strength.
    ///
    /// Near the party rather than near the anchor, which is where this
    /// parts company with `raiding_towns`: a raid is aimed at the stores and
    /// so is measured from them, and a patrol is ground the player walks
    /// into.
    ///
    /// The band is asked through `Standing::fields_patrols` every time, never
    /// cached at spawn — that is what makes a patrol stand down the moment
    /// its town stops being Hostile.
    fn towns_short_of_a_patrol(&mut self) -> Vec<(Entity, (i32, i32))> {
        let player = self.player_entity();
        let ppos = *self.world.get::<Position>(player).unwrap();
        let near: Vec<crate::settlements::SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| {
                (known.tile.0 - ppos.x)
                    .abs()
                    .max((known.tile.1 - ppos.y).abs())
                    <= SETTLEMENT_PATROL_RANGE
            })
            .map(|(key, _)| *key)
            .collect();
        let hostile: Vec<crate::settlements::SettlementKey> = near
            .into_iter()
            .filter(|&key| self.standing_band(key).fields_patrols())
            .collect();
        hostile
            .into_iter()
            .filter_map(|key| {
                let tile = self
                    .world
                    .resource::<crate::resources::Settlements>()
                    .0
                    .get(&key)?
                    .tile;
                let town = self.settlement_entity(key)?;
                (self.patrol_strength(town) < SETTLEMENT_PATROL_SIZE).then_some((town, tile))
            })
            .collect()
    }

    /// The drawable half of the town `key` names.
    ///
    /// `resources::Settlements` is the record and the entity is rebuilt from
    /// it on load, so the entity is looked up rather than stored: an
    /// `Entity` in a resource would be stale the moment a save is reloaded.
    /// Where the town *stands* comes off the record and not off this
    /// entity's `Position`, for the same reason — the record is the half
    /// that is authoritative.
    fn settlement_entity(&mut self, key: crate::settlements::SettlementKey) -> Option<Entity> {
        let mut query = self
            .world
            .query::<(Entity, &crate::components::Settlement)>();
        query
            .iter(&self.world)
            .find(|(_, settlement)| settlement.key == key)
            .map(|(entity, _)| entity)
    }

    /// How many of `town`'s patrol are still standing.
    fn patrol_strength(&mut self, town: Entity) -> u32 {
        let mut query = self.world.query::<&TownPatrol>();
        query
            .iter(&self.world)
            .filter(|patrol| patrol.town == town)
            .count() as u32
    }

    /// One member, on `town`'s own ground.
    ///
    /// `Hostile` + `WanderAi` come from `spawn_wild_creature_scaled` and
    /// `TownPatrol` is the only thing added — **exactly what a nest guardian
    /// is minus the nest**, which is what lets `pursuit_tick`,
    /// `wander_ai_system` and every combat path treat the two alike.
    ///
    /// `allow_boss: false`: a patrol is an ordinary-encounter mechanic, and
    /// a boss standing in one is a different fight. And no authored species
    /// of its own — a patrol reads as the town's from its mark and its
    /// label, not from a species nobody else fields.
    fn spawn_patrol_member(&mut self, town: Entity, tile: (i32, i32)) -> Option<Entity> {
        let (x, y) = self.patrol_stand(tile)?;
        let (species, _) = self.pick_habitat_species(x, y, None, false)?;
        // The ramp `spawn_nest_guardian` takes, for its reason: a patrol out
        // on the frontier would otherwise be weaker than the ordinary
        // wildlife it is standing among.
        let mult = self.field_stat_mult(x, y);
        let member = self.spawn_wild_creature_scaled(&species, x, y, mult, false)?;
        self.world.entity_mut(member).insert(TownPatrol { town });
        Some(member)
    }

    /// The half of a patrol that is not a nest guardian's: it notices the
    /// player by **proximity** rather than by being attacked, and it stands
    /// down the moment its town stops being Hostile.
    ///
    /// Both halves are one pass because both are answers to the same
    /// question — what is this town's opinion, *now* — and splitting them
    /// would ask it twice per member per tick.
    ///
    /// The stand-down is deliberately outside the surface guard below.
    /// A band repaired while the party is underground must have lifted by
    /// the time they climb out; a patrol frozen mid-chase by the guard is
    /// still one the player has to walk back past.
    pub(crate) fn patrol_aggro_tick(&mut self) {
        let members: Vec<(Entity, Entity, Position)> = {
            let mut query = self.world.query::<(Entity, &TownPatrol, &Position)>();
            query
                .iter(&self.world)
                .map(|(entity, patrol, &pos)| (entity, patrol.town, pos))
                .collect()
        };
        if members.is_empty() {
            return;
        }
        // Read once per tick and not once per member: a town fields several,
        // and `standing_band` is a resource lookup plus a ladder walk.
        let stood_down: Vec<Entity> = members
            .iter()
            .filter(|&&(_, town, _)| !self.town_fields_patrols(town))
            .map(|&(entity, ..)| entity)
            .collect();
        for entity in stood_down {
            // `Pursuing` goes with the tether it was inserted alongside,
            // never on its own: an untethered `Pursuing` has no leash and
            // nothing left in `pursuit_tick` can ever clear it.
            self.world
                .entity_mut(entity)
                .remove::<TownPatrol>()
                .remove::<crate::components::Pursuing>();
        }

        // `pursuit_tick`'s guard, for its reason: the player's `Position` is
        // pinned to the anchor in base space and to the entrance tile in the
        // Stack, so proximity to it means nothing while the party is out of
        // phase with it.
        if self.is_game_over().is_some()
            || self.has_active_battle()
            || self.is_underground()
            || self.in_base()
        {
            return;
        }
        let player = self.player_entity();
        let player_pos = *self.world.get::<Position>(player).unwrap();
        let noticed: Vec<Entity> = members
            .into_iter()
            .filter(|&(entity, _, _)| self.world.get::<TownPatrol>(entity).is_some())
            .filter(|&(_, _, pos)| {
                (pos.x - player_pos.x)
                    .abs()
                    .max((pos.y - player_pos.y).abs())
                    <= SETTLEMENT_PATROL_AGGRO_RADIUS
            })
            .map(|(entity, ..)| entity)
            .collect();
        for entity in noticed {
            self.world
                .entity_mut(entity)
                .insert(crate::components::Pursuing);
        }
    }

    /// What killing one of `town`'s patrol costs you with it.
    ///
    /// **That town alone, by key** — never `credit_nearby_settlements`.
    /// Routing it through the radius mover would spread a penalty whose
    /// source the player cannot see, and the neighbours have no view on
    /// whose guards died.
    ///
    /// Through `Game::adjust_standing` like every other mover, so the clamp
    /// and the only-on-a-crossing announcement are written once.
    pub(crate) fn charge_for_a_patrol_kill(&mut self, town: Entity) {
        let Some(key) = self
            .world
            .get::<crate::components::Settlement>(town)
            .map(|settlement| settlement.key)
        else {
            return;
        };
        self.adjust_standing(key, SETTLEMENT_PATROL_KILL_STANDING);
    }

    /// The name of the town `creature` patrols for, or `None` for anything
    /// that is not a patrol member.
    ///
    /// Two hops, and both are needed: the tether names an entity, the entity
    /// carries the key, and only `Settlements` knows what a key is called.
    pub(crate) fn patrol_owner(&self, creature: Entity) -> Option<String> {
        let town = self.world.get::<TownPatrol>(creature)?.town;
        let key = self.world.get::<crate::components::Settlement>(town)?.key;
        Some(self.settlement_name(key))
    }

    /// Whether the town entity `town` is still angry enough to field one.
    ///
    /// An entity rather than a key, because that is what the tether stores;
    /// a town whose entity is gone answers `false`, which stands its patrol
    /// down rather than stranding it — the belt-and-braces arm
    /// `NestGuardian` has for a nest that was destroyed, kept here for a
    /// case that should not arise.
    fn town_fields_patrols(&self, town: Entity) -> bool {
        self.world
            .get::<crate::components::Settlement>(town)
            .is_some_and(|settlement| self.standing_band(settlement.key).fields_patrols())
    }

    /// Somewhere in the ring around `tile` a patrol member can stand.
    ///
    /// **Band 0 is excluded by `SETTLEMENT_PATROL_RING_MIN`, not by a check
    /// here** — a settlement tile admits nobody (`move_player`'s fourth
    /// arm), the same reason a relay landing searches from band 1. Which is
    /// also why this cannot use `scatter_open_tile`: that one falls back to
    /// the tile it was handed, and here that is the one tile the answer may
    /// never be.
    ///
    /// One draw for where in the ring to start, then the first standable
    /// tile from there — so a town hemmed in by water fields a patrol on
    /// whatever ground it does have rather than none at all.
    fn patrol_stand(&mut self, tile: (i32, i32)) -> Option<(i32, i32)> {
        let ring = ring_tiles(tile, SETTLEMENT_PATROL_RING_MIN, SETTLEMENT_PATROL_RING_MAX);
        let start = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..ring.len())
        };
        (0..ring.len())
            .map(|step| ring[(start + step) % ring.len()])
            .find(|&(x, y)| {
                self.world
                    .resource_mut::<WorldMap>()
                    .tile(x, y)
                    .open_to_hostiles()
            })
    }
}
