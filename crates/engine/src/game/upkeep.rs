//! Per-tick base maintenance: structure regeneration, nest respawns,
//! visual effects, and raids.

use crate::tuning::{
    RAID_CHANCE_PER_TICK, RAID_DAMAGE, RAID_DEFENDER_DAMAGE, STRUCTURE_REGEN_INTERVAL,
};
use crate::*;

impl Game {
    /// Repairs damaged structures — every `STRUCTURE_REGEN_INTERVAL` ticks,
    /// every structure below max `Durability` recovers whatever the base's
    /// repairers restore between them (`total_repair_rate`).
    ///
    /// That total is the only source: nothing heals on its own, so a base
    /// with no repairer standing never recovers a point and raid damage is
    /// permanent until the player builds something that undoes it.
    ///
    /// `With<Structure>` is load-bearing, not tidiness: a `Nest` carries
    /// `Durability` too, and an unfiltered pass healed it alongside the
    /// player's own buildings — so chipping a nest down with bump-attacks
    /// raced its own regeneration. Nothing the player builds maintains what
    /// spawns the raiders; a nest's Durability is only ever spent.
    pub(crate) fn structure_regen(&mut self) {
        let tick = self.world.resource::<GameClock>().tick;
        if !tick.is_multiple_of(STRUCTURE_REGEN_INTERVAL) {
            return;
        }
        let amount = self.total_repair_rate();
        if amount == 0 {
            return;
        }
        let mut query = self
            .world
            .query_filtered::<&mut Durability, With<Structure>>();
        for mut durability in query.iter_mut(&mut self.world) {
            durability.hp = (durability.hp + amount).min(durability.max_hp);
        }
    }

    /// `Durability` restored to every deployed structure per regen interval
    /// by the base's repairers — each one's `RepairDef::per_tier` times its
    /// own `StructureTier`, summed. Derived on each call rather than cached,
    /// so a Patch Node lost to a raid stops contributing with no
    /// invalidation step, the same way `pet_capacity` handles a lost Data
    /// Cache.
    pub(crate) fn total_repair_rate(&mut self) -> u32 {
        let repairers: Vec<(StructureId, u32)> = {
            let mut query = self.world.query::<(&Structure, Option<&StructureTier>)>();
            query
                .iter(&self.world)
                .map(|(s, tier)| (s.kind.clone(), tier.map_or(1, |t| t.0)))
                .collect()
        };
        let db = self.world.resource::<StructureDb>();
        repairers
            .iter()
            .filter_map(|(kind, tier)| Some((db.get(kind.as_str())?.repair?, tier)))
            .map(|(repair, tier)| repair.per_tier * tier)
            .sum()
    }

    /// Advances every `Nest`'s `pending_respawns` countdown by one tick,
    /// spawning a replacement guardian for each entry that reaches 0 (a
    /// nest can have more than one entry reach 0 on the same tick, e.g.
    /// two guardians killed together, so this spawns once per ready
    /// entry, not once per nest). Called directly from `tick` —
    /// not registered on `self.schedule` — because it needs
    /// `spawn_nest_guardian`, a `Game` method unreachable from a bevy
    /// system function.
    pub(crate) fn nest_respawn_tick(&mut self) {
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
                // A replacement spawned mid-siege — some other guardian of
                // this nest still bears `Pursuing` from the player's last
                // hit — arrives already provoked, rather than standing
                // there calm until the next swing reaches it.
                if let Some(guardian) = self.spawn_nest_guardian(nest, &species, pos.x, pos.y)
                    && self.nest_has_pursuers(nest)
                {
                    self.world.entity_mut(guardian).insert(Pursuing);
                }
            }
        }
    }

    /// Rolls `RAID_CHANCE_PER_TICK`; on success, picks one deployed
    /// structure at random and either damages it directly (undefended) or
    /// has its assigned cronjob worker, if any, fight the raid off —
    /// reducing the structure's damage by the worker's Defense, at the
    /// cost of `RAID_DEFENDER_DAMAGE` to the worker. A worker knocked to 0
    /// HP stands down from the cronjob (like a knocked-out companion, not
    /// destroyed — `rest` heals it back up along with every other tamed
    /// program you own). A structure whose `Durability` reaches 0 is
    /// destroyed and any cronjob assignment on it is dropped.
    /// Total raid-damage reduction contributed by every deployed structure
    /// with `StructureDef::raid_defense` set (e.g. a Shield) — a base-wide
    /// network, not tied to any one structure. Destroying one of these
    /// structures in a raid naturally shrinks this, since it's recomputed
    /// fresh from whatever's still standing.
    /// Drains every `VisualEffect` queued since the last call — the visual
    /// counterpart to `App::take_sounds`. A frontend without effects can
    /// drop the result, but must still call it so the queue doesn't sit at
    /// its cap.
    pub fn take_effects(&mut self) -> Vec<VisualEffect> {
        self.world.resource_mut::<EffectQueue>().take()
    }

    /// Queues `kind` at `structure`'s tile, if it has one. Raid targets are
    /// selected by `With<Durability>`, which doesn't imply `Position` —
    /// a flash on the wrong tile would be worse than none, so a positionless
    /// entity queues nothing.
    pub(crate) fn push_effect(&mut self, structure: Entity, kind: EffectKind) {
        let Some(pos) = self.world.get::<Position>(structure).map(|p| (p.x, p.y)) else {
            return;
        };
        self.world.resource_mut::<EffectQueue>().push(pos, kind);
    }

    /// Whether any deployed structure contributes raid defense — the seam
    /// frontends use to show the shield network as active without reaching
    /// into `StructureDb` themselves.
    pub fn raid_defense_active(&self) -> bool {
        self.total_raid_defense() > 0
    }

    pub(crate) fn total_raid_defense(&self) -> u32 {
        let structure_db = self.world.resource::<StructureDb>();
        self.world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>())
            .filter_map(|s| structure_db.get(&s.kind))
            .map(|def| def.raid_defense)
            .sum()
    }

    pub(crate) fn raid_check(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(RAID_CHANCE_PER_TICK)
        };
        if !roll {
            return;
        }
        let targets: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, (With<Durability>, Without<Nest>)>();
            query.iter(&self.world).collect()
        };
        if targets.is_empty() {
            return;
        }
        let target = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..targets.len());
            targets[idx]
        };
        let target_label = self.entity_label(target);
        let raid_damage = RAID_DAMAGE.saturating_sub(self.total_raid_defense());

        let defender = {
            let mut query = self.world.query::<(Entity, &Task)>();
            query
                .iter(&self.world)
                .find(|(_, t)| t.target == target)
                .map(|(e, _)| e)
        };

        let Some(worker) = defender else {
            if raid_damage > 0 {
                self.damage_structure(target, raid_damage, &target_label);
            } else {
                self.push_effect(target, EffectKind::Deflected);
                self.log_base(format!(
                    "Your shield network fends off a raid on {target_label} without a scratch!"
                ));
            }
            return;
        };

        let worker_def = self.world.get::<Stats>(worker).map(|s| s.def).unwrap_or(0);
        let mitigated = raid_damage.saturating_sub(worker_def.max(0) as u32);
        let worker_label = self.creature_label(worker);
        if mitigated > 0 {
            self.damage_structure(target, mitigated, &target_label);
        } else {
            self.push_effect(target, EffectKind::Deflected);
            self.log_base(format!(
                "{worker_label} fends off a raid on {target_label} without a scratch!"
            ));
        }
        self.apply_damage(worker, RAID_DEFENDER_DAMAGE);
        if !self.creature_alive(worker) {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{worker_label} is destroyed defending {target_label}."),
            );
            // Stripped before the dissolve, not by it. `raid_check` finds its
            // defender *by* this `Task`, so the program is always working the
            // structure the line above already names — leaving the `Task` on
            // would have `sale_detachments` add a redundant "stops working
            // the Mining Node" directly beneath it.
            self.world.entity_mut(worker).remove::<Task>();
            self.dissolve_tamed_program(worker);
        }
    }

    /// Applies `dmg` to `structure`'s `Durability`, destroying (despawning)
    /// it and clearing any cronjob assignment pointing at it if that
    /// brings it to 0.
    pub(crate) fn damage_structure(&mut self, structure: Entity, dmg: u32, label: &str) {
        let Some(mut durability) = self.world.get_mut::<Durability>(structure) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        // Queued before the despawn below, which takes the `Position` the
        // effect needs with it.
        self.push_effect(
            structure,
            if destroyed {
                EffectKind::Destroyed
            } else {
                EffectKind::Hit
            },
        );
        if destroyed {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{label} is destroyed in a raid!"),
            );
            let workers: Vec<Entity> = {
                let mut query = self.world.query::<(Entity, &Task)>();
                query
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == structure)
                    .map(|(e, _)| e)
                    .collect()
            };
            for w in workers {
                self.world.entity_mut(w).remove::<Task>();
            }
            self.announce_lost_shelf(structure);
            self.world.despawn(structure);
        } else {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{label} takes {dmg} raid damage!"),
            );
        }
    }
}
