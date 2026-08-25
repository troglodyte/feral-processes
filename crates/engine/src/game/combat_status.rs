//! Status conditions, combat buffs and field buffs — arming them, reading
//! them, and ticking them down.
//!
//! Damage lives in `combat_damage.rs`, enemy behaviour in `combat_enemy.rs`
//! and battle teardown in `combat_teardown.rs`; this file is what those
//! three read and write *through*.

use crate::tuning::DEFEND_MITIGATION_BONUS;
use crate::*;

impl Game {
    /// Whether `entity` is holding the Defend stance this round.
    pub(crate) fn is_defending(&self, entity: Entity) -> bool {
        self.world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .is_some_and(|a| a.kind == BuffKind::Mitigation && a.power == DEFEND_MITIGATION_BONUS)
    }

    /// Rolls `effect.chance`; on success, overwrites `target`'s active
    /// status condition (see `StatusEffects`) and logs it. A miss is
    /// silent — the move's direct damage still landed, it just didn't also
    /// inflict its status this time.
    ///
    /// `kind` is the log styling for the condition line, taken from the caller
    /// because both sides of a fight inflict conditions and only the caller
    /// knows which one this is.
    pub(crate) fn apply_status_effect(
        &mut self,
        target: Entity,
        effect: &species::MoveEffect,
        target_label: &str,
        kind: MessageKind,
    ) {
        let applied = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(effect.chance as f64)
        };
        if !applied {
            return;
        }
        self.arm_status(target, effect.kind, effect.duration, effect.power);
        match effect.kind {
            StatusKind::Bleed => self.log_kind(
                kind,
                format!("{target_label} starts bleeding corrupted data!"),
            ),
            StatusKind::Stun => self.log_kind(kind, format!("{target_label} locks up, stunned!")),
            StatusKind::Exposed => {
                self.log_kind(kind, format!("{target_label} is left wide open!"))
            }
        }
    }

    /// Arms `kind` on `entity` for `duration` rounds, overwriting whatever
    /// condition it was carrying (see `StatusEffects` — one slot, no
    /// stacking) and marking it as landed this round.
    ///
    /// The only writer of `StatusEffects::active` outside the two that
    /// clear it (`Cleanse` and `end_battle`), which is what makes
    /// `ActiveStatus::landed_this_round` true for every condition that has
    /// ever been inflicted rather than for the ones whose call site
    /// remembered. Silently does nothing on an entity with no
    /// `StatusEffects` component — a condition needs somewhere to live, and
    /// the caller has no better answer than the target simply not being a
    /// combatant.
    pub(crate) fn arm_status(
        &mut self,
        entity: Entity,
        kind: StatusKind,
        duration: u32,
        power: i32,
    ) {
        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(entity) {
            statuses.active = Some(ActiveStatus {
                kind,
                remaining: duration,
                power,
                landed_this_round: true,
            });
        }
    }

    /// Whether `entity` currently has an active `Stun` status. Doesn't
    /// consume it — the end-of-round tick does that, and the round the stun
    /// landed in is exempt (see `ActiveStatus::landed_this_round`), so a
    /// stun always survives into a round the victim has yet to act in.
    pub(crate) fn is_stunned(&self, entity: Entity) -> bool {
        self.world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
            .is_some_and(|a| a.kind == StatusKind::Stun)
    }

    /// End-of-round status upkeep for one combatant: `Bleed` deals its
    /// damage, then the active effect's remaining-rounds counter ticks
    /// down, clearing it once it hits 0.
    ///
    /// The first call after a condition is armed does neither, and only
    /// clears `ActiveStatus::landed_this_round` — see that field for why a
    /// condition's own landing round must not be charged to it.
    pub(crate) fn tick_status_effects(&mut self, entity: Entity, label: &str) {
        let Some(active) = self
            .world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
        else {
            return;
        };

        if active.landed_this_round {
            if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(entity) {
                statuses.active = Some(ActiveStatus {
                    landed_this_round: false,
                    ..active
                });
            }
            return;
        }

        if active.kind == StatusKind::Bleed {
            self.apply_damage(entity, active.power);
            self.log(format!("{label} takes {} bleed damage.", active.power));
        }

        let remaining = active.remaining.saturating_sub(1);
        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(entity) {
            statuses.active = if remaining == 0 {
                None
            } else {
                Some(ActiveStatus {
                    remaining,
                    ..active
                })
            };
        }
        if remaining == 0 {
            match active.kind {
                StatusKind::Bleed => self.log(format!("{label}'s bleed clears.")),
                StatusKind::Stun => self.log(format!("{label} shakes off the stun.")),
                StatusKind::Exposed => self.log(format!("{label} recovers its guard.")),
            }
        }
    }

    /// End-of-round upkeep for one combatant's active combat buff (see
    /// `CombatBuff`) — ticks its remaining-rounds counter down, clearing it
    /// (with a log line) once it expires.
    pub(crate) fn tick_combat_buff(&mut self, entity: Entity) {
        let Some(mut buff) = self.world.get_mut::<CombatBuff>(entity) else {
            return;
        };
        let Some(active) = buff.active else {
            return;
        };
        let remaining = active.remaining.saturating_sub(1);
        buff.active = if remaining == 0 {
            None
        } else {
            Some(ActiveBuff {
                remaining,
                ..active
            })
        };
        if remaining == 0 {
            let stat = match active.kind {
                BuffKind::Atk => "attack",
                BuffKind::Mitigation => "mitigation",
            };
            if entity == self.player_entity() {
                self.log(format!("Your {stat} boost fades."));
            } else {
                let name = self.creature_label(entity);
                self.log(format!("{name}'s {stat} boost fades."));
            }
        }
    }

    /// Counts one round off every cooldown `entity` is carrying, dropping
    /// the entries that reach zero so the map doesn't grow across a long
    /// fight.
    pub(crate) fn tick_ability_cooldowns(&mut self, entity: Entity) {
        let Some(mut cooldowns) = self.world.get_mut::<AbilityCooldowns>(entity) else {
            return;
        };
        cooldowns.0.retain(|_, remaining| {
            *remaining = remaining.saturating_sub(1);
            *remaining > 0
        });
    }

    /// Arms `buff` on `entity`, inserting the slot if it has none. Only the
    /// player is spawned holding a `CombatBuff`, so anything that buffs a
    /// companion has to go through here or it silently changes nothing.
    ///
    /// A single `active` slot, so this overwrites whatever the target was
    /// still carrying — a real cost of the choice.
    pub(crate) fn arm_buff(&mut self, entity: Entity, buff: ActiveBuff) {
        match self.world.get_mut::<CombatBuff>(entity) {
            Some(mut existing) => existing.active = Some(buff),
            None => {
                self.world
                    .entity_mut(entity)
                    .insert(CombatBuff { active: Some(buff) });
            }
        }
    }

    /// Arms `buff` on `entity`, inserting `FieldBuff` if it has none — only
    /// the player is spawned holding one, the same gap `arm_buff` fills for
    /// `CombatBuff`.
    ///
    /// The only writer of `FieldBuff::active`, and the sole place either of
    /// its two collision rules is enforced: a `Consumable` displaces
    /// whatever consumable buff was running, of any kind, since an item
    /// slot holds one effect; a `Routine` displaces only a running
    /// `Routine` of the *same* kind, so recasting one routine leaves every
    /// other one alone.
    pub(crate) fn arm_field_buff(&mut self, entity: Entity, buff: ActiveFieldBuff) {
        if self.world.get::<FieldBuff>(entity).is_none() {
            self.world.entity_mut(entity).insert(FieldBuff::default());
        }
        let mut field_buff = self.world.get_mut::<FieldBuff>(entity).unwrap();
        match buff.source {
            BuffSource::Consumable => field_buff
                .active
                .retain(|b| b.source != BuffSource::Consumable),
            BuffSource::Routine => field_buff
                .active
                .retain(|b| !(b.source == BuffSource::Routine && b.kind == buff.kind)),
        }
        field_buff.active.push(buff);
    }

    /// `entity`'s running `kind` field buff power, `0` when it has no
    /// `FieldBuff` at all (an entity that has never had one armed) — see
    /// `components::field_buff_power_of` for what happens when it does.
    pub(crate) fn field_buff_power(&self, entity: Entity, kind: FieldBuffKind) -> i32 {
        self.world
            .get::<FieldBuff>(entity)
            .map(|field_buff| crate::components::field_buff_power_of(field_buff, kind))
            .unwrap_or(0)
    }

    /// Ends every until-rest buff on the player and each party member,
    /// logging one line per drop in the same words an expiry uses — from the
    /// player's side a loadout ending because they slept is the same event as
    /// one ending because its clock ran out.
    ///
    /// Walks the same player-plus-`Party` set `tick_field_buffs` does, and
    /// for the same reason: it is the only set an invocation will ever arm one on.
    /// `Game::rest` is one of the two callers; the other is
    /// `difficulty::death_handling_system`, which is a bevy system and so
    /// reaches `drop_until_rest_buffs` directly instead of coming through
    /// here.
    pub(crate) fn drop_until_rest_buffs_on_party(&mut self) {
        let player = self.player_entity();
        let subjects: Vec<Entity> = std::iter::once(player)
            .chain(self.world.resource::<Party>().0.clone())
            .collect();

        let mut dropped: Vec<String> = Vec::new();
        for entity in subjects {
            if let Some(mut field_buff) = self.world.get_mut::<FieldBuff>(entity) {
                dropped.extend(components::drop_until_rest_buffs(&mut field_buff));
            }
        }
        for name in dropped {
            self.log(format!("{name} fades."));
        }
    }

    /// Applies every field buff's per-tick effect (see
    /// `apply_field_buff_tick`) on the player and each party member, then
    /// ages every buff by one tick, dropping any that just hit zero and
    /// logging one line per expiry — named from `ActiveFieldBuff::name`,
    /// not `kind`, since two different routines can arm the same kind and
    /// the buff list has to say which one actually faded.
    ///
    /// The two passes are deliberately separate: applying first means a
    /// buff with one tick left still does its last tick of work, since the
    /// pass that decrements and removes it hasn't run yet.
    ///
    /// Scoped to the player and party, unlike `age_temporary_structures`
    /// (`game/turn.rs`), which walks every `Temporary` entity that exists.
    /// This is not just where a field buff happens to end up — it is the
    /// only set `Game::run_field_routine` will ever arm one on: a
    /// `Creature`-scoped `OneAlly` run is checked against this same
    /// player-plus-`Party` roster before anything is armed, so nothing this
    /// loop skips is a buff that's actually running anywhere.
    pub(crate) fn tick_field_buffs(&mut self) {
        let player = self.player_entity();
        let subjects: Vec<Entity> = std::iter::once(player)
            .chain(self.world.resource::<Party>().0.clone())
            .collect();

        for &entity in &subjects {
            let Some(field_buff) = self.world.get::<FieldBuff>(entity) else {
                continue;
            };
            // A buff with an `interval` fires only on the turns its cadence
            // lands on — see `ActiveFieldBuff::interval` for why the phase
            // comes off `remaining` rather than a counter of its own. The
            // `max(1)` is not defensive tidiness: `interval` reaches here
            // from a `.ron` a mod wrote, and a zero would panic the game on
            // the modulus.
            let ticking: Vec<(FieldBuffKind, i32)> = field_buff
                .active
                .iter()
                .filter(|b| b.remaining % b.interval.max(1) == 0)
                .map(|b| (b.kind, b.power))
                .collect();
            for (kind, power) in ticking {
                self.apply_field_buff_tick(entity, kind, power);
            }
        }

        let mut expired: Vec<String> = Vec::new();
        for entity in subjects {
            let Some(mut field_buff) = self.world.get_mut::<FieldBuff>(entity) else {
                continue;
            };
            field_buff.active.retain_mut(|buff| {
                // An until-rest buff has no turn count to spend — `rest` and
                // a Forgiving reboot are what end it, through
                // `drop_until_rest_buffs`. Skipping the decrement rather than
                // the whole entry keeps the cadence filter above untouched:
                // only `Regen` and `Trickle` fire per tick, and neither is
                // ever an until-rest kind.
                if buff.runs_until_rest() {
                    return true;
                }
                buff.remaining = buff.remaining.saturating_sub(1);
                if buff.remaining == 0 {
                    expired.push(buff.name.clone());
                    false
                } else {
                    true
                }
            });
        }
        for name in expired {
            self.log(format!("{name} fades."));
        }
    }

    /// One tick of a single running field buff's effect on `entity`. The
    /// three over-time kinds write here; the rate and flat combat-stat
    /// kinds have no per-tick effect of their own — they're read on demand
    /// by `field_buff_power` instead — so they fall through the wildcard.
    ///
    /// `Regen` is a heal, not damage, so it writes `Stats::hp` directly
    /// rather than going through `apply_damage` — that function is the
    /// only path that *lowers* HP, and routing a heal through it would
    /// break that invariant. `Trickle` writes `PowerReserve`, which only
    /// the player has (`FieldBuffKind::scope` makes both `Run`-scoped for
    /// exactly that reason) — a companion carrying one is not an error, the
    /// write simply has nothing to land on.
    ///
    /// `Regen` is gated on `creature_alive` first: `tick_field_buffs` runs
    /// on every `tick()`, including every battle round, and `Party`
    /// deliberately keeps a dead member around until `end_battle` reaps
    /// them (`BattleState::planned` indexes `Party` positionally, so
    /// nothing may leave mid-battle). A `Regen` with no floor check would
    /// heal that corpse back to positive HP on the very next tick — this
    /// repo shipped permadeath, so an accidental auto-revive is a real bug,
    /// not a curiosity. `apply_damage` is still the only path that lowers
    /// HP; this is the same invariant's other edge, a heal with a floor.
    fn apply_field_buff_tick(&mut self, entity: Entity, kind: FieldBuffKind, power: i32) {
        match kind {
            FieldBuffKind::Regen => {
                if self.creature_alive(entity)
                    && let Some(mut stats) = self.world.get_mut::<Stats>(entity)
                {
                    stats.hp = (stats.hp + power).min(stats.max_hp);
                }
            }
            FieldBuffKind::Trickle => {
                if let Some(mut needs) = self.world.get_mut::<PowerReserve>(entity) {
                    needs.restore(power as f32);
                }
            }
            FieldBuffKind::Atk
            | FieldBuffKind::Mitigation
            | FieldBuffKind::CaptureBoost
            | FieldBuffKind::XpBoost
            | FieldBuffKind::EncounterDamp
            | FieldBuffKind::DropBoost => {}
        }
    }

    /// Braces `entity` for the round: a mitigation bonus that expires in the same
    /// round's `tick_round_status_effects`. The buff slot is inserted on
    /// demand — only the player is spawned holding one, so without this a
    /// companion's Defend would log its message and change nothing.
    pub(crate) fn begin_defend(&mut self, entity: Entity) {
        self.arm_buff(
            entity,
            ActiveBuff {
                kind: BuffKind::Mitigation,
                remaining: 1,
                power: DEFEND_MITIGATION_BONUS,
            },
        );
        let name = self.creature_label(entity);
        self.log(format!("{name} braces against the next strike."));
    }

    /// End-of-round status upkeep across every combatant in the fight: each
    /// living enemy in every group, the player, and every party member,
    /// plus each of their combat buffs. Anything a lingering Bleed finished
    /// off is then cleared out of its group.
    pub(crate) fn tick_round_status_effects(&mut self, player: Entity) {
        for wild in self.all_living_enemies() {
            let label = self.entity_label(wild);
            self.tick_status_effects(wild, &label);
            // Both of these were party-only while abilities were party-only.
            // A carrier's routine has to cool or it fires once and never
            // again, and a mirrored buff has to expire or its `duration` is
            // decoration.
            self.tick_combat_buff(wild);
            self.tick_ability_cooldowns(wild);
        }
        let player_label = self.entity_label(player);
        self.tick_status_effects(player, &player_label);
        self.tick_combat_buff(player);
        self.tick_ability_cooldowns(player);
        for companion in self.world.resource::<Party>().0.clone() {
            let label = self.creature_label(companion);
            self.tick_status_effects(companion, &label);
            self.tick_combat_buff(companion);
            self.tick_ability_cooldowns(companion);
        }
        if self.reap_dead_members(player) {
            return;
        }
        if !self.creature_alive(player) {
            let front = self.front_of_group(0);
            self.end_battle(player, front);
        }
    }
}
