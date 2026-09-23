//! Per-tick base maintenance: structure regeneration, nest respawns,
//! visual effects, and raids.

use crate::alerts::AlertKind;
use crate::components::{Downed, MemorySubject};
use crate::species::AffinityClass;
use crate::tuning::{
    BASTION_DEF_MULTIPLIER, MEDIC_REPAIR_PER_INTERVAL, RAID_DAMAGE, RAID_DEFENDER_DAMAGE,
    RAID_MIN_BASE_STAFF, RAID_MIN_ZONE, RAID_PRESSURE_JITTER_PERCENT, RAID_PRESSURE_PER_ZONE,
    RAID_PRESSURE_THRESHOLD, RAID_PRESSURE_WARN_PERCENT, STRUCTURE_REGEN_INTERVAL,
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

    /// Accrues `resources::RaidPressure`; on crossing its drawn interval,
    /// picks one deployed
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

    /// Drains every `BoltCue` queued since the last call — `take_transits`'
    /// counterpart on a battle map.
    ///
    /// A frontend that draws no bolts must still call it, or the queue sits
    /// at its cap forever. A cue names *board* cells, so one drawn anywhere
    /// but the tactical pane is the same cross-space aliasing `take_transits`
    /// warns about one space over.
    pub fn take_bolts(&mut self) -> Vec<crate::resources::BoltCue> {
        self.world
            .resource_mut::<crate::resources::BoltQueue>()
            .take()
    }

    /// Drains every `TacticalFxCue` queued since the last call — a body's
    /// own hit or heal on a battle map, `take_bolts`' counterpart for the
    /// blow itself rather than the streak that travelled to it.
    ///
    /// A frontend that draws none must still call it, or the queue sits at
    /// its cap forever — `take_bolts`' reason exactly.
    pub fn take_tactical_fx(&mut self) -> Vec<crate::resources::TacticalFxCue> {
        self.world
            .resource_mut::<crate::resources::TacticalFxQueue>()
            .take()
    }

    /// Drains the band of every swing resolved on a battle map since the
    /// last call — what a frontend plays, one cue a blow, the moment it
    /// lands. `App::take_sounds` is the one caller, and it must be called
    /// every frame for `take_bolts`' reason.
    pub fn take_swing_cues(&mut self) -> Vec<crate::resources::SwingOutcome> {
        self.world
            .resource_mut::<crate::resources::SwingCueQueue>()
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
        let structures: u32 = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>())
            .filter_map(|s| structure_db.get(&s.kind))
            .map(|def| def.raid_defense)
            .sum();
        structures + self.garrison_defense()
    }

    /// What the party's friendly neighbours station around the base — the
    /// aid ladder's passive half, `Standing::garrison_defense` summed over
    /// every known town within `SETTLEMENT_GARRISON_RADIUS` of the anchor.
    ///
    /// **The clamp is on this sum alone and never on the total.** Clamping
    /// the total would cap the player's own shield network with a settlement
    /// constant, which is a different feature and a regression — two Shields
    /// already out-defend `SETTLEMENT_GARRISON_MAX` and must keep doing so.
    /// Leaving it off entirely is worse: `run_raid` subtracts this from
    /// `RAID_DAMAGE` with `saturating_sub`, so enough Allied neighbours take
    /// every sweep to zero while still logging one, and raids stop happening
    /// in everything but the message log.
    ///
    /// Which towns are near enough to count is `town_garrisons`' question,
    /// not this one's — the page has to ask it per town, so the radius check
    /// lives there and this is the fold over it.
    fn garrison_defense(&self) -> u32 {
        let towns: Vec<crate::settlements::SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .keys()
            .copied()
            .collect();
        towns
            .into_iter()
            .map(|key| self.town_garrisons(key))
            .sum::<u32>()
            .min(crate::tuning::SETTLEMENT_GARRISON_MAX)
    }

    /// What one town contributes to the fold above — its band's answer if it
    /// is near enough to the anchor to station anyone, and zero otherwise.
    ///
    /// Split out because the town page has to say whether *this* town
    /// garrisons, and `garrison_defense` returns a clamped sum that cannot
    /// answer for one. Both sides call this rather than restating the radius
    /// check, so a new condition on the fold cannot leave the page promising
    /// a detachment that no longer arrives.
    pub(crate) fn town_garrisons(&self, key: crate::settlements::SettlementKey) -> u32 {
        let Some((ax, ay)) = self.anchor_position() else {
            return 0;
        };
        let Some(tile) = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
        else {
            return 0;
        };
        if (tile.0 - ax).abs().max((tile.1 - ay).abs()) > crate::tuning::SETTLEMENT_GARRISON_RADIUS
        {
            return 0;
        }
        self.standing_band(key).garrison_defense()
    }

    /// Every known town near enough to the anchor and angry enough to send
    /// raiders at it — the hostile mirror of `garrison_defense`'s fold, and
    /// deliberately a *list* rather than a count, because this event has an
    /// author and the log line has to name it.
    ///
    /// Two filters and a discovery rule. The radius is Chebyshev to the
    /// anchor, as the garrison's is. The band is asked through
    /// `Standing::sends_raiders`, never restated here. And a town whose
    /// tile has never been resolved is absent from `Settlements` entirely,
    /// so it is excluded by construction rather than by a third check —
    /// `town_garrisons`' rule, and the same reason: aid and hostility both
    /// follow discovery.
    ///
    /// Order is `Settlements`' own `BTreeMap` order, which is stable across
    /// a save round trip. `town_raid_check` picks from this with one draw
    /// and would otherwise be seed-unstable.
    pub(crate) fn raiding_towns(&self) -> Vec<crate::settlements::SettlementKey> {
        let Some((ax, ay)) = self.anchor_position() else {
            return Vec::new();
        };
        let near: Vec<crate::settlements::SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| {
                (known.tile.0 - ax).abs().max((known.tile.1 - ay).abs())
                    <= crate::tuning::SETTLEMENT_RAID_RADIUS
            })
            .map(|(key, _)| *key)
            .collect();
        near.into_iter()
            .filter(|&key| self.standing_band(key).sends_raiders())
            .collect()
    }

    /// Fires a GC Entropy Sweep now, without waiting on the clock — the dev
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

    /// Winds the clock to its own approach warning without firing a sweep,
    /// so the console can watch the half `dev_force_raid` skips past.
    ///
    /// **Winds to the drawn interval's warn point, not to a fixed number**,
    /// which is the only way one press works whatever the jitter rolled: a
    /// literal high enough to guarantee a warning on the shortest interval
    /// is past the *target* on it, and the row would sweep rather than warn.
    /// So the interval is drawn here if it has not been drawn yet, and the
    /// level is set as a share of it.
    ///
    /// Sets the level rather than ticking in a loop, because the loop would
    /// be two thousand ticks of every other base pass and what the console
    /// is asking for is the clock, not the base.
    #[doc(hidden)]
    pub fn dev_wind_raid_clock(&mut self) {
        let target = match self
            .world
            .resource::<crate::resources::RaidPressure>()
            .next_at
        {
            Some(target) => target,
            None => {
                let target = self.draw_raid_interval();
                self.world
                    .resource_mut::<crate::resources::RaidPressure>()
                    .next_at = Some(target);
                target
            }
        };
        let mut pressure = self.world.resource_mut::<crate::resources::RaidPressure>();
        pressure.level = target * RAID_PRESSURE_WARN_PERCENT / 100;
        pressure.warned = false;
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
        self.damage_structure(target, hp, &label, "a GC Entropy Sweep");
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
        self.damage_structure(target, dmg, &label, "a GC Entropy Sweep");
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

    /// How close the base is to its next sweep, in `resources::RaidPressure`'s
    /// own units. The dev console's reading and every test's.
    pub fn raid_pressure(&self) -> u32 {
        self.world
            .resource::<crate::resources::RaidPressure>()
            .level
    }

    pub(crate) fn raid_check(&mut self) {
        let zone = self.world.resource::<ZoneLevel>().0;
        // **The sector gate is on accrual and not on firing**, which is where
        // it sat when this was a roll. Pressure the opening sector could
        // build would be pressure it could never spend, so a player crossing
        // into sector 2 would be swept within a tick of arriving for a
        // quietness they had already served.
        if zone < RAID_MIN_ZONE {
            return;
        }

        let target = match self
            .world
            .resource::<crate::resources::RaidPressure>()
            .next_at
        {
            Some(target) => target,
            None => {
                let target = self.draw_raid_interval();
                self.world
                    .resource_mut::<crate::resources::RaidPressure>()
                    .next_at = Some(target);
                target
            }
        };

        let level = {
            let mut pressure = self.world.resource_mut::<crate::resources::RaidPressure>();
            pressure.level += RAID_PRESSURE_PER_ZONE * zone;
            pressure.level
        };

        // Latched on the resource rather than re-read, because the
        // condition stays true for the rest of the interval: as a bare
        // inequality this line is said twice a second for the whole warning
        // window. Cleared by the reset a sweep does, so each interval warns
        // once.
        if level >= target * RAID_PRESSURE_WARN_PERCENT / 100
            && !self
                .world
                .resource::<crate::resources::RaidPressure>()
                .warned
        {
            self.world
                .resource_mut::<crate::resources::RaidPressure>()
                .warned = true;
            let text = "Sweep telemetry thickens around the anchor. A GC Entropy Sweep is forming."
                .to_string();
            self.log_base_kind(MessageKind::Raid, text.clone());
            self.post_alert(AlertKind::SweepIncoming, "sweep", text);
        }

        if level < target {
            return;
        }
        // **The staff floor gates the sweep and the clock keeps its
        // pressure.** As a gate on accrual it would be the old roll's
        // behaviour — a base that benches its crew is never swept — and as a
        // reset it would forgive the whole interval. Held, the sweep simply
        // waits for a base that can absorb it, which is what the floor was
        // always for.
        if self.defending_base_staff_count() < RAID_MIN_BASE_STAFF {
            return;
        }
        // **The clock is spent by a sweep, not by reaching the threshold.**
        // `run_raid` answers `false` when there is nothing standing to
        // sweep, and resetting on that would rewind the meter every tick a
        // base is bare — so the first machine a player raises would buy them
        // a fresh interval they had not served.
        if !self.run_raid() {
            return;
        }
        let mut pressure = self.world.resource_mut::<crate::resources::RaidPressure>();
        pressure.level = 0;
        pressure.warned = false;
        pressure.next_at = None;
    }

    /// One interval, jittered, in `resources::RaidPressure`'s units.
    ///
    /// The run's only `GameRng` draw on this meter, and it costs one per
    /// *sweep* rather than one per tick — see `RAID_PRESSURE_JITTER_PERCENT`
    /// for why jittering the target beats jittering the accrual.
    fn draw_raid_interval(&mut self) -> u32 {
        let low = RAID_PRESSURE_THRESHOLD * (100 - RAID_PRESSURE_JITTER_PERCENT) / 100;
        let high = RAID_PRESSURE_THRESHOLD * (100 + RAID_PRESSURE_JITTER_PERCENT) / 100;
        let mut rng = self.world.resource_mut::<GameRng>();
        rng.0.random_range(low..=high)
    }

    /// The roll for a town-sourced raid, and the one caller that decides one
    /// happens.
    ///
    /// **Roll first, gate after** — `raid_check`'s and
    /// `maybe_spawn_wild_creature`'s shared discipline: the number of draws
    /// a tick costs is a constant, so it cannot depend on what the world
    /// happens to hold. Note what that does and does not buy — it keeps the
    /// stream stable across *worlds*, not across *versions*: this check is a
    /// second unconditional draw per tick, and adding it moved the stream
    /// for every seeded test that ticks.
    ///
    /// **Neither `RAID_MIN_ZONE` nor `RAID_MIN_BASE_STAFF` applies, and both
    /// omissions are deliberate.** The zone floor exists so a player who has
    /// not engaged the game is not swept; reaching `Hostile` with a town
    /// near the anchor *is* engagement, and gating it on depth would make
    /// the consequence of a choice wait on an unrelated axis. The staff
    /// floor stops a base already reduced to wreckage from being ground
    /// down — but this raid breaks nothing, so there is no attrition spiral
    /// for it to prevent, and a base with nobody on shift is exactly the one
    /// whose stores are easiest to walk off with.
    pub(crate) fn town_raid_check(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool(crate::tuning::SETTLEMENT_RAID_CHANCE_PER_TICK)
        };
        if !roll {
            return;
        }
        let candidates = self.raiding_towns();
        if candidates.is_empty() {
            return;
        }
        // One draw, over an order `raiding_towns` documents as stable.
        let key = {
            let mut rng = self.world.resource_mut::<GameRng>();
            candidates[rng.0.random_range(0..candidates.len())]
        };
        self.run_town_raid(key);
    }

    /// How many base-staff programs (`Game::base_staff`) could actually
    /// defend the base — the population `RAID_MIN_BASE_STAFF` is measured
    /// against. A body that cannot defend or absorb the next raid must not
    /// be counted toward the floor, or a base already reduced to wreckage
    /// keeps taking hits.
    ///
    /// **Two states, not one, and the names are the trap.**
    /// `components::Downed` is a program that died and was benched;
    /// `Disgruntled { DownedTools }` is one whose morale ran far enough
    /// under that it stopped working. Neither will lift a finger, and only
    /// the first was checked here at first — so a base whose whole staff had
    /// downed tools read as fully staffed and was swept until its machines
    /// were gone, which is exactly the failure the floor exists to prevent.
    ///
    /// The morale half is `has_downed_tools` **called**, not restated: it is
    /// the same question `schedule_base_labour`'s `on_shift` filter asks,
    /// and a copy here would be the one that forgets `Grievance` has two
    /// rungs. A `Sulking` program still works and still counts.
    pub(crate) fn defending_base_staff_count(&self) -> usize {
        self.defending_base_staff().len()
    }

    /// `defending_base_staff_count`'s own population, as the list rather
    /// than the count — `Game::resolve_siege_offscreen` needs actual bodies
    /// to bench, not just how many there are, and a second filter here
    /// would be the copy that eventually forgets `Grievance` has two rungs.
    pub(crate) fn defending_base_staff(&self) -> Vec<Entity> {
        self.base_staff()
            .into_iter()
            .filter(|&e| self.world.get::<Downed>(e).is_none())
            .filter(|&e| !self.has_downed_tools(e))
            .collect()
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
    /// A sweep that landed no damage, whichever way it was turned aside.
    ///
    /// One hook where the two deflection branches meet — the shield network's
    /// and the posted defender's — rather than the deed at each of them: a
    /// hook that has to be repeated is aimed at the wrong seam.
    fn sweep_held(&mut self, target: Entity) {
        self.push_effect(target, EffectKind::Deflected);
        self.note_deed(crate::contracts::Deed::RepelledRaid);
    }

    fn run_raid(&mut self) -> bool {
        // `StructureDef::swept` narrows the pool, never widens it: the
        // `Durability` filter still says what can be damaged at all, and
        // an unswept structure keeps its pool for a siege to spend.
        let targets: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Structure), With<Durability>>();
            let db = self.world.resource::<StructureDb>();
            query
                .iter(&self.world)
                .filter(|(_, s)| db.get(s.kind.as_str()).is_none_or(|def| def.swept))
                .map(|(e, _)| e)
                .collect()
        };
        if targets.is_empty() {
            return false;
        }
        // After the empty-targets return, or a base with nothing raidable
        // (only a Home, `raidable: false`) gets a fresh alert on every
        // `raid_check` past the threshold — the pressure never resets when
        // nothing is swept, so every later check would re-post.
        self.post_alert(
            AlertKind::SweepHit,
            "sweep",
            "A GC Entropy Sweep hits the base.",
        );
        // Here rather than in `raid_check`, which can decide a sweep happens
        // and then find nothing standing to sweep. This is the first line
        // after a sweep is real.
        self.notify(crate::notifications::NotificationKind::FirstRaid);
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
                self.damage_structure(target, raid_damage, &target_label, "a GC Entropy Sweep");
            } else {
                self.sweep_held(target);
                self.log_base(format!(
                    "Your shield network fends off a GC Entropy Sweep on {target_label} without a scratch!"
                ));
            }
            return true;
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
            self.damage_structure(target, mitigated, &target_label, "a GC Entropy Sweep");
        } else {
            self.sweep_held(target);
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
        true
    }

    /// Everything a town raid *is*, once it has been decided one happens.
    ///
    /// Split from the roll for `run_raid`'s reason: the console fires the
    /// real thing, and the decision stays with the one caller that should
    /// be making it.
    ///
    /// **It takes rather than breaks**, which is the whole of what makes
    /// this a different event from a sweep. The four economy roles are
    /// separate on purpose; raiders at the *base* take what the *base* runs
    /// on, so the cost is construction. Progression is earned by fighting
    /// and is deliberately untouched.
    pub(crate) fn run_town_raid(&mut self, key: crate::settlements::SettlementKey) {
        let name = self.settlement_name(key);
        // Before the outcome branches, `run_raid`'s placement: the lesson is
        // "something you did caused this", and a raid turned away is still a
        // raid that happened.
        self.notify(crate::notifications::NotificationKind::FirstTownRaid);

        let cut = self.total_raid_defense() * crate::tuning::SETTLEMENT_RAID_DEFENSE_PER_POINT;
        let percent = crate::tuning::SETTLEMENT_RAID_HAUL_PERCENT.saturating_sub(cut);
        // **Before the floor, and the order is load-bearing.** The floor
        // exists so a share of a *small bank* does not round to nothing; run
        // after a defense that already drove the share to zero it would hand
        // the raiders a unit anyway, delete the deflect outcome, and leave
        // the log claiming a shield network that had stopped working. Two
        // different zeroes, and only one of them is the floor's business.
        if percent == 0 {
            if let Some(defender) = self.first_raid_defender() {
                self.push_effect(defender, EffectKind::Deflected);
            }
            self.log_base_kind(
                MessageKind::Raid,
                format!("Raiders out of {name} probe your defences and turn back empty-handed."),
            );
            return;
        }

        let currency = self.currency();
        let money = self.item_name(&currency).to_string();
        let banked = self.banked(&currency);
        let want = (banked * percent / 100).clamp(
            crate::tuning::SETTLEMENT_RAID_HAUL_FLOOR,
            crate::tuning::SETTLEMENT_RAID_HAUL_CAP,
        );
        let player = self.player_entity();
        // `Inventory::take` is the third bound: it takes what is there and
        // reports it, so an empty store is an outcome rather than an
        // underflow.
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .map(|mut inv| inv.take(currency, want))
            .unwrap_or(0);

        if taken == 0 {
            self.log_base_kind(
                MessageKind::Raid,
                format!("Raiders out of {name} ransack your stores and find them bare."),
            );
            return;
        }
        self.log_base_kind(
            MessageKind::Raid,
            format!("Raiders out of {name} carry off {taken} {money} from your stores."),
        );
    }

    /// Which standing structure a deflected town raid flashes over — the
    /// lowest-tiled one that actually contributes `raid_defense`.
    ///
    /// **A `Structure` and never the anchor.** A `VisualEffect` names a
    /// base-space cell (`render/base.rs` draws the queue only while `base_pos`
    /// is `Some`), and the anchor is a zone-surface fixture whose tile is also
    /// the party's pinned `Position` out of phase — so a flash on it paints
    /// the player's own cell, which is exactly the cross-space aliasing that
    /// gate was added to close.
    /// `seam:a-raids-flash-is-base-space-too-and-the-pane-has-to-say-so`
    /// records the ambient sweep suppressing its flash rather than moving it
    /// there; this moves it onto something that is genuinely in the right
    /// space instead.
    ///
    /// **A deflect always has one.** Turning a raid away needs
    /// `SETTLEMENT_RAID_DEFENSE_PER_POINT * defense >= SETTLEMENT_RAID_HAUL_PERCENT`,
    /// so defense of at least 5, while the settlement half is clamped at
    /// `SETTLEMENT_GARRISON_MAX` (3) — a garrison alone can never reach the
    /// branch that calls this. The `Option` is honesty about the signature,
    /// not a case the game reaches.
    ///
    /// Sorted by tile, `run_repair_bays`' rule: bevy's query iteration order
    /// is not stable, so two Shields would otherwise flash different cells
    /// between runs.
    fn first_raid_defender(&mut self) -> Option<Entity> {
        let defended: Vec<(String, Entity, (i32, i32))> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Structure, &Position), With<Durability>>();
            query
                .iter(&self.world)
                .map(|(e, s, p)| (s.kind.clone(), e, (p.x, p.y)))
                .collect()
        };
        let structure_db = self.world.resource::<StructureDb>();
        defended
            .into_iter()
            .filter(|(kind, _, _)| {
                structure_db
                    .get(kind)
                    .is_some_and(|def| def.raid_defense > 0)
            })
            .min_by_key(|(_, _, tile)| *tile)
            .map(|(_, entity, _)| entity)
    }

    /// Fires a town raid now, skipping the roll — the dev console's door and
    /// the only way a test reaches a `SETTLEMENT_RAID_CHANCE_PER_TICK`
    /// event. `dev_force_raid`'s precedent exactly: it calls the real body,
    /// so the console cannot disagree with the game about the haul, the
    /// defense cut or the lines.
    ///
    /// Reachable only through the `FERAL_DEV_CONSOLE` gate.
    #[doc(hidden)]
    pub fn dev_force_town_raid(&mut self, key: crate::settlements::SettlementKey) {
        self.run_town_raid(key);
    }

    /// Applies `dmg` to `structure`'s `Durability`, destroying (despawning)
    /// it and clearing any cronjob assignment pointing at it if that
    /// brings it to 0.
    ///
    /// `event` is the log line's noun phrase for what did this — `"a GC
    /// Entropy Sweep"` for every raid caller, `"a siege"` for
    /// `Game::resolve_siege_offscreen` and the tactical board's structure
    /// swing — so a structure destroyed by one event is never reported as
    /// destroyed by the other. The teardown itself (tasks cleared, the rig's
    /// tool returned, the pending build cleared, the memory formed) is
    /// identical either way, which is what "destroyed the way any other one
    /// is" means.
    pub(crate) fn damage_structure(
        &mut self,
        structure: Entity,
        dmg: u32,
        label: &str,
        event: &str,
    ) {
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
                format!("{label} is destroyed in {event}!"),
            );
            for w in workers {
                // See `remove_structure`: the load has to go with the task,
                // and this is the second of the two destruction paths — so
                // an in-transit carrier is put back here too, before the
                // component goes with it.
                self.return_carried_program(w);
                self.world.entity_mut(w).remove::<(Task, Carrying)>();
            }
            // The second of the two destruction paths for the rig's tool
            // too — see `Game::return_rig_tool`.
            self.return_rig_tool(structure);
            // The second of the two destruction paths — see
            // `Game::clear_pending_build_at`. A machine swept out from under
            // its own pending upgrade leaves the units already carried there
            // standing on a cell nothing occupies.
            if let Some(pos) = self.world.get::<Position>(structure).copied() {
                self.clear_pending_build_at(pos.x, pos.y);
            }
            // The second of the two destruction paths for a subject pinned
            // in this structure's pen — see `Game::release_study_station`.
            self.release_study_station(structure);
            self.announce_lost_shelf(structure);
            self.world.despawn(structure);
        } else {
            self.log_base_kind(
                MessageKind::Raid,
                format!("{label} loses {dmg} Durability to {event}!"),
            );
        }
    }
}
