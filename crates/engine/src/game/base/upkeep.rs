//! Per-tick base maintenance: structure regeneration, nest respawns,
//! visual effects, and raids.

use crate::components::MemorySubject;
use crate::species::AffinityClass;
use crate::tuning::{
    BASTION_DEF_MULTIPLIER, MEDIC_REPAIR_PER_INTERVAL, RAID_CHANCE_PER_TICK, RAID_DAMAGE,
    RAID_DEFENDER_DAMAGE, STRUCTURE_REGEN_INTERVAL,
};
use crate::*;

/// How much of a structure's *maximum* Durability one forced hit takes.
///
/// A percentage rather than a flat figure so the row behaves the same on a
/// Home as on a Mining Node, and so repeated presses converge on 1 HP rather
/// than eventually destroying the thing being watched — see
/// `repeated_forced_hits_never_destroy_the_structure`.
///
/// Deliberately not in `tuning.rs`: that file is how hard the game is, and
/// nothing a player can reach reads this.
pub(crate) const DEV_HIT_DAMAGE_PERCENT: u32 = 25;

impl Game {
    /// The class `creature` is read as, or `None` for the player, for a
    /// species the db has never heard of, and for anything outside the class
    /// system (a boss, or a mod raising two affinity axes).
    ///
    /// The one door from an entity to its base job, so the two jobs that
    /// happen to a *posted* program — mitigating a sweep and repairing what
    /// one took — cannot disagree about who qualifies.
    pub(crate) fn creature_class(&self, creature: Entity) -> Option<AffinityClass> {
        let species = &self.world.get::<Creature>(creature)?.species;
        self.world
            .resource::<SpeciesDb>()
            .get(species)?
            .affinity_class()
    }

    /// Repairs damaged structures — every `STRUCTURE_REGEN_INTERVAL` ticks,
    /// every structure below max `Durability` recovers whatever the base's
    /// repairers restore between them (`total_repair_rate`), and each
    /// structure a Medic is posted to recovers
    /// `MEDIC_REPAIR_PER_INTERVAL` more.
    ///
    /// Those two are the only sources: nothing heals on its own, so a base
    /// with neither a repairer standing nor a Medic posted never recovers a
    /// point and raid damage is permanent.
    ///
    /// The two differ in reach on purpose. A Patch Node is a *building* and
    /// works base-wide from wherever it stands; a Medic is a *program* and
    /// mends the one structure it is guarding, so posting it is a decision
    /// about what to protect rather than a rate added to a pool. Which is
    /// also why the early return has to ask about both — a base with no
    /// Patch Node at all is the case a posted Medic is most for.
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
        let base_wide = self.total_repair_rate();
        let mended = self.medic_posts();
        if base_wide == 0 && mended.is_empty() {
            return;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &mut Durability), With<Structure>>();
        for (structure, mut durability) in query.iter_mut(&mut self.world) {
            let amount = base_wide
                + MEDIC_REPAIR_PER_INTERVAL
                    * mended.iter().filter(|&&e| e == structure).count() as u32;
            durability.hp = (durability.hp + amount).min(durability.max_hp);
        }
    }

    /// The structures a Medic is currently guarding, one entry per posted
    /// Medic — `displace_task_holder` allows a single `Guard` per structure,
    /// so today that is one entry each, but counting rather than
    /// de-duplicating means a second route to a shared post would stack
    /// rather than silently do nothing.
    ///
    /// `TaskKind::Guard` and not any task pointing at the structure, which
    /// is deliberately narrower than the sweep defender `run_raid` picks:
    /// mitigating a sweep is a passive property of whoever happens to be
    /// standing there, while mending is *what the post is*. A Medic running
    /// a cronjob is extracting, not repairing, and that is the cost that
    /// makes posting one a decision.
    fn medic_posts(&mut self) -> Vec<Entity> {
        let posts: Vec<(Entity, Entity)> = {
            let mut query = self.world.query::<(Entity, &Task)>();
            query
                .iter(&self.world)
                .filter(|(_, task)| task.kind == TaskKind::Guard)
                .map(|(worker, task)| (worker, task.target))
                .collect()
        };
        posts
            .into_iter()
            .filter(|&(worker, _)| self.creature_class(worker) == Some(AffinityClass::Medic))
            .map(|(_, structure)| structure)
            .collect()
    }

    /// `Durability` restored to every deployed structure per regen interval
    /// by the base's repairers — each one's `RepairDef::per_tier` times its
    /// own `StructureTier`, summed. Derived on each call rather than cached,
    /// so a Patch Node lost to a raid stops contributing with no
    /// invalidation step, the same way `pet_capacity` handles a lost Data
    /// Cache.
    pub(crate) fn total_repair_rate(&mut self) -> u32 {
        let perk = crate::perks::repair_rate_bonus(self.player_perks());
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
            .sum::<u32>()
            + perk
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

    /// Drains every `TransitCue` queued since the last call — `take_effects`'
    /// counterpart for a body walking across base space rather than something
    /// happening on one tile.
    ///
    /// A frontend that draws no walks must still call it so the queue does
    /// not sit at its cap, and one that is not currently drawing base space
    /// drops what it gets: a cue names base-space cells, and painting it over
    /// the zone surface is the cross-space aliasing `base_pos` already gates
    /// raid flashes against.
    pub fn take_transits(&mut self) -> Vec<crate::resources::TransitCue> {
        self.world
            .resource_mut::<crate::resources::TransitQueue>()
            .take()
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

    /// Fires a GC Entropy Sweep now, skipping the per-tick roll — the dev
    /// console's trigger.
    ///
    /// Calls `run_raid` rather than carrying its own copy of the body, so
    /// what the console puts on screen is evidence about the sweep a player
    /// actually meets. Gated by `FERAL_DEV_CONSOLE` at the app-core layer,
    /// which is the only thing that reaches it.
    #[doc(hidden)]
    pub fn dev_force_raid(&mut self) {
        self.run_raid();
    }

    /// Destroys the structure nearest the player outright, through the same
    /// `damage_structure` a sweep uses so every consequence of a building
    /// coming down still happens.
    ///
    /// Exists because wearing one structure to zero in play takes hundreds
    /// of ticks: sweeps pick a random target and do `RAID_DAMAGE` at a time.
    #[doc(hidden)]
    pub fn dev_destroy_structure(&mut self) {
        let Some(target) = self.nearest_damageable_structure() else {
            return;
        };
        let label = self.entity_label(target);
        let hp = self
            .world
            .get::<Durability>(target)
            .map(|d| d.hp)
            .unwrap_or(0);
        self.damage_structure(target, hp, &label);
    }

    /// Wounds the structure nearest the player without destroying it, which
    /// is the only way to reach the `EffectKind::Hit` branch on demand —
    /// `dev_destroy_structure` deals full `Durability` and so always lands
    /// on `Destroyed`.
    #[doc(hidden)]
    pub fn dev_damage_structure(&mut self) {
        let Some(target) = self.nearest_damageable_structure() else {
            return;
        };
        let Some(durability) = self.world.get::<Durability>(target).copied() else {
            return;
        };
        // Held one short of lethal rather than clamped after the fact: the
        // row exists to be pressed repeatedly at the thing you are watching,
        // and a press that destroyed it would end the very effect it is
        // there to show.
        let dmg = (durability.max_hp * DEV_HIT_DAMAGE_PERCENT / 100)
            .max(1)
            .min(durability.hp.saturating_sub(1));
        let label = self.entity_label(target);
        self.damage_structure(target, dmg, &label);
    }

    /// The structure a dev trigger acts on: nearest to the player, ties
    /// broken by id so a press resolves the same way every time rather than
    /// on bevy's query iteration order.
    ///
    /// `With<Structure>` for `repair_system`'s reason, and it is what the
    /// target pool is stated as: `Durability` alone has never meant "a
    /// building". A `Nest` carries one and is wildlife; a `DigSite` carries
    /// one and is marked rock. A trigger meant for a machine must pick a
    /// machine, and the positive filter keeps a fifth `Durability` carrier
    /// out by construction rather than needing a fourth exclusion.
    fn nearest_damageable_structure(&mut self) -> Option<Entity> {
        let at = self.world.get::<Position>(self.player_entity()).copied()?;
        let mut targets: Vec<(Entity, i32)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), (With<Durability>, With<Structure>)>();
            query
                .iter(&self.world)
                .map(|(e, p)| (e, (p.x - at.x).abs() + (p.y - at.y).abs()))
                .collect()
        };
        targets.sort_by_key(|(e, d)| (*d, e.to_bits()));
        targets.first().map(|(e, _)| *e)
    }

    pub(crate) fn raid_check(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(RAID_CHANCE_PER_TICK)
        };
        if !roll {
            return;
        }
        self.run_raid();
    }

    /// Everything a sweep *is*, once it has been decided that one happens.
    ///
    /// Split from the roll so the dev console can fire the real thing. The
    /// roll stays in `raid_check` because that is the only caller that
    /// should be making the decision.
    ///
    /// The target pool is `With<Structure>` and not merely `Durability`, for
    /// the reason `nearest_damageable_structure` states. A marked box is up
    /// to 625 `DigSite`s, each carrying `Durability`; under the old filter
    /// they swamped the real machines, and a sweep that destroyed one
    /// dropped the mark and every swing of chip progress while `BaseGrid`
    /// still reported the cell solid — the wall silently healed to full.
    fn run_raid(&mut self) {
        let targets: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, (With<Durability>, With<Structure>)>();
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
                    "Your shield network fends off a GC Entropy Sweep on {target_label} without a scratch!"
                ));
            }
            return;
        };

        let worker_mitigation = self
            .world
            .get::<Stats>(worker)
            .map(|s| s.mitigation)
            .unwrap_or(0);
        // The Bastion base job. Every posted program mitigates by its
        // Mitigation — the defender above is found by `Task::target`, not by
        // `TaskKind::Guard` — so what the buff class brings is that the
        // number counts twice.
        let worker_mitigation = match self.creature_class(worker) {
            Some(AffinityClass::Bastion) => worker_mitigation * BASTION_DEF_MULTIPLIER,
            _ => worker_mitigation,
        };
        // A percentage cut, since that is what `Stats::mitigation` now is —
        // subtracting it from a durability figure would make a raid defender
        // worth almost nothing.
        //
        // **Clamped to 100 rather than to `MAX_MITIGATION_PERCENT`, and that
        // is deliberate.** The combat cap exists so no creature reaches
        // immunity to attacks; a raid is not an attack on a creature, and
        // "fends off a sweep without a scratch" is a shipped outcome with its
        // own log line and `EffectKind::Deflected`. Capping at 75 here would
        // delete that outcome silently, and with it the whole point of the
        // Bastion's doubling.
        let cut = worker_mitigation.clamp(0, 100) as f32 / 100.0;
        let mitigated = (raid_damage as f32 * (1.0 - cut)).round() as u32;
        let worker_label = self.creature_label(worker);
        if mitigated > 0 {
            self.damage_structure(target, mitigated, &target_label);
        } else {
            self.push_effect(target, EffectKind::Deflected);
            self.log_base(format!(
                "{worker_label} fends off a GC Entropy Sweep on {target_label} without a scratch!"
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
            self.bench_or_dissolve(worker);
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
        // Both hoisted above the branch, and both for the destroyed side's
        // sake: it despawns the structure the kind is read off, and the
        // surviving side never looked at who was standing here at all.
        let kind = self
            .world
            .get::<Structure>(structure)
            .map(|s| s.kind.clone());
        let workers: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &Task)>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.target == structure)
                .map(|(e, _)| e)
                .collect()
        };
        // A sweep is remembered by whoever was posted at what it hit, on
        // **both** branches: being caught at a machine that survived and
        // being caught at one that did not are the same thing to the body
        // standing there, and only the second was ever visible here.
        //
        // The subject is the machine's *kind* and not the entity, so the
        // memory outlives the structure — which is what lets it be formed on
        // the branch that is about to despawn it, and what makes a rebuilt
        // Lathe the same Lathe to a program that was hurt at one.
        if let Some(kind) = kind {
            for &w in &workers {
                self.remember(w, "swept_here", MemorySubject::Structure(kind.clone()));
            }
        }
        if destroyed {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{label} is destroyed in a GC Entropy Sweep!"),
            );
            for w in workers {
                // See `remove_structure`: the load has to go with the task,
                // and this is the second of the two destruction paths.
                self.world.entity_mut(w).remove::<(Task, Carrying)>();
            }
            // The second of the two destruction paths — see
            // `Game::clear_pending_build_at`. A machine swept out from under
            // its own pending upgrade leaves the units already carried there
            // standing on a cell nothing occupies.
            if let Some(pos) = self.world.get::<Position>(structure).copied() {
                self.clear_pending_build_at(pos.x, pos.y);
            }
            self.announce_lost_shelf(structure);
            self.world.despawn(structure);
        } else {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{label} loses {dmg} Durability to a GC Entropy Sweep!"),
            );
        }
    }
}
