//! Per-tick base maintenance: structure regeneration, nest respawns,
//! visual effects, and raids.

use crate::*;

impl Game {
    /// Slow passive healing for damaged structures — every
    /// `STRUCTURE_REGEN_INTERVAL` ticks, everything below max `Durability`
    /// recovers `STRUCTURE_REGEN_AMOUNT`.
    pub(crate) fn structure_regen(&mut self) {
        let tick = self.world.resource::<GameClock>().tick;
        if !tick.is_multiple_of(STRUCTURE_REGEN_INTERVAL) {
            return;
        }
        let mut query = self.world.query::<&mut Durability>();
        for mut durability in query.iter_mut(&mut self.world) {
            durability.hp = (durability.hp + STRUCTURE_REGEN_AMOUNT).min(durability.max_hp);
        }
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
                self.spawn_nest_guardian(nest, &species, pos.x, pos.y);
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
                self.log(format!(
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
            self.log(format!(
                "{worker_label} fends off a raid on {target_label} without a scratch!"
            ));
        }
        self.apply_damage(worker, RAID_DEFENDER_DAMAGE);
        if !self.creature_alive(worker) {
            self.log(format!(
                "{worker_label} is knocked offline defending {target_label} and stands down from its cronjob."
            ));
            self.world.entity_mut(worker).remove::<Task>();
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
            self.log_kind(
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
            self.world.despawn(structure);
        } else {
            self.log_kind(
                MessageKind::Raid,
                format!("{label} takes {dmg} raid damage!"),
            );
        }
    }
}
