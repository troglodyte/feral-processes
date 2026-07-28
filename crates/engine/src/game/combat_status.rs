//! Damage, status effects, enemy retaliation, and tearing a battle down —
//! whether it ended in a win, a flee, or a loss.

use crate::tuning::{
    DEFEND_DEF_BONUS, ENGAGED_GROUPS, FLEE_COUNTERATTACK_CHANCE, JACK_OUT_LUCK_MAX,
    JACK_OUT_LUCK_MIN, WILD_ABILITY_CHANCE,
};
use crate::*;

impl Game {
    /// Attempts to jack out, returning whether the party actually got clear.
    ///
    /// The escape is a roll, not a given — `battle::jack_out_chance` weighs
    /// your side's summed power against the pack's, times a luck draw. A
    /// failed attempt burns the round: every engaged group swings, the
    /// round counter advances and end-of-round upkeep runs, but no XP is
    /// docked. You pay the setback only for an escape you actually got,
    /// which is what stops repeated attempts from bleeding progression on
    /// top of HP.
    pub fn battle_flee(&mut self) -> bool {
        if self.is_game_over().is_some() {
            return false;
        }
        let Some(player) = self.world.get_resource::<BattleState>().map(|b| b.player) else {
            return false;
        };
        let luck = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(JACK_OUT_LUCK_MIN..=JACK_OUT_LUCK_MAX)
        };
        let chance =
            battle::jack_out_chance(self.party_side_power(), self.enemy_side_power(), luck);
        let escaped = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if !escaped {
            self.log("The exit route collapses — they're still on you!");
            self.all_wild_retaliate(player);
            // The attempt cost the whole party its round, so the fight
            // advances exactly as a resolved round does — same upkeep, same
            // counter. `tick_round_status_effects` is also what ends the
            // battle if that volley flatlined the player.
            if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
                battle.round += 1;
                let slots = battle.planned.len();
                battle.planned = vec![None; slots];
            }
            self.tick_round_status_effects(player);
            self.tick();
            return false;
        }
        let got_hit = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(FLEE_COUNTERATTACK_CHANCE)
        };
        if got_hit {
            self.log_kind(
                MessageKind::Outcome,
                "You jack out, but not before taking a parting counter-strike!",
            );
            self.all_wild_retaliate(player);
        } else {
            self.log_kind(MessageKind::Outcome, "You jack out safely.");
        }
        // A forced jack-out costs a little progress too — nothing drastic,
        // same mild setback as a flatline (see `death_handling_system`).
        if let Some(mut exp) = self.world.get_mut::<Experience>(player) {
            let xp_lost = progression::apply_setback_xp_penalty(&mut exp);
            if xp_lost > 0 {
                self.log_kind(
                    MessageKind::Outcome,
                    format!("Bailing out costs you {xp_lost} XP."),
                );
            }
        }
        let front = self.front_of_group(0);
        self.end_battle(player, front);
        self.tick();
        true
    }

    /// Whether `entity` is holding the Defend stance this round.
    pub(crate) fn is_defending(&self, entity: Entity) -> bool {
        self.world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .is_some_and(|a| a.kind == BuffKind::Def && a.power == DEFEND_DEF_BONUS)
    }

    /// Weighted target roll across the player and every living party
    /// member: front slots draw more fire than back ones, and a bracing
    /// member draws more still. Soft ranks — every member stays targetable,
    /// slot order only changes the odds.
    pub(crate) fn roll_enemy_target(&mut self, player: Entity) -> Entity {
        let party = self.world.resource::<Party>().0.clone();
        let mut pool: Vec<(Entity, u32)> = Vec::new();
        for (slot, entity) in std::iter::once(player).chain(party).enumerate() {
            if !self.creature_alive(entity) {
                continue;
            }
            let weight = crate::battle::slot_aggro_weight(slot, self.is_defending(entity));
            pool.push((entity, weight));
        }
        let total: u32 = pool.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return player;
        }
        let mut roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..total)
        };
        for (entity, weight) in &pool {
            if roll < *weight {
                return *entity;
            }
            roll -= weight;
        }
        player
    }

    /// The wild creature strikes back at whoever's exposed: normally the
    /// player or a party member, weighted by slot — see `roll_enemy_target`.
    pub(crate) fn wild_retaliate(&mut self, wild: Entity, group: usize, player: Entity) {
        let species_id = self.world.get::<Creature>(wild).unwrap().species.clone();
        // Only the front `ENGAGED_GROUPS` are close enough to swing; anything
        // further back has to shoot, and idles if it has nothing that reaches.
        let engaged = group < ENGAGED_GROUPS;
        let candidates: Vec<MoveDef> = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .unwrap()
            .moves
            .iter()
            .filter(|m| engaged || m.ranged)
            .cloned()
            .collect();
        if candidates.is_empty() {
            let name = self.creature_label(wild);
            self.log(format!("{name} circles beyond reach, unable to strike."));
            return;
        }
        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..candidates.len())
        };
        let mut mv = candidates[idx].clone();
        // A moveset's status effects are what a program *can* bring to bear,
        // not what it does every turn. Reaching for one every time meant a
        // species with a nasty stun was that stun on repeat.
        //
        // Gates the effect only — the move still lands its full damage — so
        // this changes how a fight *feels* without touching the damage
        // curves `balance_sim` projects.
        //
        // Composes with the move's own `effect.chance` rather than replacing
        // it: that figure is per-move `.ron` data, including anyone's mods,
        // and is the move's own reliability. Shipped chances are 0.3-0.5, so
        // an effect actually lands on roughly 6-10% of wild attacks.
        let reaches_for_effect = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(WILD_ABILITY_CHANCE)
        };
        if !reaches_for_effect {
            mv.effect = None;
        }

        let target = self.roll_enemy_target(player);
        let targets_companion = target != player;

        let (w_atk, t_def) = {
            let w = *self.world.get::<Stats>(wild).unwrap();
            (w.atk, self.effective_def(target))
        };
        let dmg = battle::compute_damage(w_atk, t_def, mv.power);
        self.apply_damage(target, dmg);

        // A move that also inflicts a condition is the only thing an enemy has
        // resembling the party's Special, so it is what earns the louder
        // colour in the log.
        //
        // Read *after* the gate above has had its say, deliberately: the
        // colour then means "it reached for the effect this turn", not "this
        // move theoretically has one". Taken before the gate, a Wraith would
        // read as a special on every swing while the condition landed on
        // barely one in ten of them.
        let kind = if mv.effect.is_some() {
            MessageKind::EnemySpecial
        } else {
            MessageKind::EnemyAttack
        };

        if targets_companion {
            let name = self.creature_label(target);
            self.log_kind(
                kind,
                format!(
                    "The rogue program executes {} on {} for {} damage.",
                    mv.name, name, dmg
                ),
            );
            if !self.creature_alive(target) {
                self.log(format!("{name} is knocked offline and stands down."));
                // It leaves `Party` at the end of the battle, not here —
                // `BattleState::planned` indexes `Party` positionally, so
                // removing a member mid-battle shifts everyone behind it
                // into the wrong slot. `slot_can_act` is what keeps the
                // empty-handed slot from holding the round open until then.
            } else if let Some(effect) = &mv.effect {
                self.apply_status_effect(target, effect, &name, kind);
            }
        } else {
            self.log_kind(
                kind,
                format!("The rogue program executes {} for {} damage.", mv.name, dmg),
            );
            if self.creature_alive(target)
                && let Some(effect) = &mv.effect
            {
                self.apply_status_effect(target, effect, "You", kind);
            }
        }
    }

    pub(crate) fn apply_damage(&mut self, target: Entity, dmg: i32) {
        if let Some(mut stats) = self.world.get_mut::<Stats>(target) {
            stats.hp = (stats.hp - dmg).max(0);
        }
    }

    pub(crate) fn creature_alive(&self, e: Entity) -> bool {
        self.world
            .get::<Stats>(e)
            .map(|s| s.hp > 0)
            .unwrap_or(false)
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
        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(target) {
            statuses.active = Some(ActiveStatus {
                kind: effect.kind,
                remaining: effect.duration,
                power: effect.power,
            });
        }
        match effect.kind {
            StatusKind::Bleed => self.log_kind(
                kind,
                format!("{target_label} starts bleeding corrupted data!"),
            ),
            StatusKind::Stun => self.log_kind(kind, format!("{target_label} locks up, stunned!")),
        }
    }

    /// Whether `entity` currently has an active `Stun` status. Doesn't
    /// consume it — `consume_stun` does that once a round is confirmed to
    /// skip.
    pub(crate) fn is_stunned(&self, entity: Entity) -> bool {
        self.world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
            .is_some_and(|a| a.kind == StatusKind::Stun)
    }

    /// End-of-round status upkeep for one combatant: `Bleed` deals its
    /// damage, then the active effect's remaining-rounds counter ticks
    /// down, clearing it once it hits 0.
    pub(crate) fn tick_status_effects(&mut self, entity: Entity, label: &str) {
        let Some(active) = self
            .world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
        else {
            return;
        };

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
                BuffKind::Def => "defense",
            };
            if entity == self.player_entity() {
                self.log(format!("Your {stat} boost fades."));
            } else {
                let name = self.creature_label(entity);
                self.log(format!("{name}'s {stat} boost fades."));
            }
        }
    }

    /// Braces `entity` for the round: a DEF bonus that expires in the same
    /// round's `tick_round_status_effects`. The buff slot is inserted on
    /// demand — only the player is spawned holding one, so without this a
    /// companion's Defend would log its message and change nothing.
    /// Arms `buff` on `entity`, inserting the slot if it has none. Only the
    /// player is spawned holding a `CombatBuff`, so anything that buffs a
    /// companion has to go through here or it silently changes nothing.
    ///
    /// A single `active` slot, so this overwrites whatever the target was
    /// still carrying — a real cost of the choice.
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

    pub(crate) fn begin_defend(&mut self, entity: Entity) {
        self.arm_buff(
            entity,
            ActiveBuff {
                kind: BuffKind::Def,
                remaining: 1,
                power: DEFEND_DEF_BONUS,
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

    /// Clears out any group whose front died to a status tick, awarding
    /// loot and XP exactly as a direct kill would. Walks back to front so
    /// removing a group can't shift a later one out from under the loop.
    /// Returns whether that ended the battle.
    pub(crate) fn reap_dead_members(&mut self, player: Entity) -> bool {
        let mut group = self.living_group_count();
        while group > 0 {
            group -= 1;
            let mut index = self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| b.groups.get(group))
                .map(|g| g.members.len())
                .unwrap_or(0);
            while index > 0 {
                index -= 1;
                let alive = self
                    .world
                    .get_resource::<BattleState>()
                    .and_then(|b| b.groups.get(group))
                    .and_then(|g| g.members.get(index))
                    .is_some_and(|&e| self.creature_alive(e));
                if alive {
                    continue;
                }
                if self.finish_member(group, index, player) {
                    return true;
                }
            }
        }
        false
    }

    /// Clears any residual status effects from the player, `wild`, and
    /// every party member, and the player's active combat buff (see
    /// `CombatBuff`). Status conditions are scoped to a single intrusion, so
    /// nothing should carry forward once one ends, however it ends. `wild`
    /// is `None` when the pack is already gone, and may name an entity that
    /// is already despawned (a kill), in which case clearing it is a no-op —
    /// neither case may skip clearing your own side.
    pub(crate) fn clear_battle_status_effects(&mut self, player: Entity, wild: Option<Entity>) {
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(player) {
            s.active = None;
        }
        if let Some(mut b) = self.world.get_mut::<CombatBuff>(player) {
            b.active = None;
        }
        if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(player) {
            c.0.clear();
        }
        if let Some(mut s) = wild.and_then(|w| self.world.get_mut::<StatusEffects>(w)) {
            s.active = None;
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(companion) {
                s.active = None;
            }
            // Companions hold `CombatBuff` too, now that a Rally or Shield
            // can be aimed at one. Left set, it never ticks down outside a
            // battle and `effective_def`/`effective_atk` read it
            // unconditionally, so it would be a permanent free stat.
            if let Some(mut b) = self.world.get_mut::<CombatBuff>(companion) {
                b.active = None;
            }
            if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(companion) {
                c.0.clear();
            }
        }
    }

    /// Tears the current battle down: every combat-only effect cleared from
    /// both sides, companions knocked offline during the fight finally stood
    /// down, and `BattleState` dropped.
    ///
    /// Standing the fallen down happens here rather than the moment they
    /// fall because `BattleState::planned` indexes `Party` positionally (see
    /// `actor_entity`) — removing a member mid-battle shifts every member
    /// behind it into the wrong slot.
    ///
    /// `wild` is passed in rather than looked up because two of the callers
    /// have already popped the group: the entity whose status must be
    /// cleared is the one that just died or was decompiled, not whatever
    /// stepped up behind it. A freshly tamed program joining the party still
    /// Bleeding is the bug this guards.
    pub(crate) fn end_battle(&mut self, player: Entity, wild: Option<Entity>) {
        self.clear_battle_status_effects(player, wild);
        let downed: Vec<Entity> = self
            .world
            .resource::<Party>()
            .0
            .iter()
            .copied()
            .filter(|&e| !self.creature_alive(e))
            .collect();
        if !downed.is_empty() {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|e| !downed.contains(e));
        }
        // The blow-by-blow has done its job by now — the battle pane showed
        // it live. What follows the player onto the map is the results.
        self.world
            .resource_mut::<MessageLog>()
            .retain_outcomes_since_battle();
        self.world.remove_resource::<BattleState>();
    }
}
