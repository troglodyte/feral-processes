//! Resolving a planned round — running each actor's action, computing
//! effective stats, and rendering the result as a `BattleView`.

use crate::tuning::{
    DEFAULT_TAMING_DIFFICULTY, ENGAGED_GROUPS, FRONT_SLOTS, NEST_RESPAWN_TICKS,
    PARTY_PASSIVE_STAT_DIVISOR, PLAYER_STRIKE_POWER,
};
use crate::*;

impl Game {
    /// A planned target group may have died earlier in the round. Fall back
    /// to the lowest surviving group rather than wasting the turn.
    pub(crate) fn retarget(&self, group: usize) -> Option<usize> {
        let count = self.living_group_count();
        if count == 0 {
            None
        } else if group < count {
            Some(group)
        } else {
            Some(0)
        }
    }

    /// Resolves the planned round: everyone rolls initiative, acts in
    /// order, and the plan is cleared for the next round. A no-op unless
    /// every slot is planned.
    pub fn battle_resolve_round(&mut self) {
        if !self.battle_round_ready() || self.is_game_over().is_some() {
            return;
        }
        // Read before the increment at the end of this method, so the number
        // matches the planning screen's own "round N" header.
        let round = self.world.resource::<BattleState>().round;
        // The pane shows one round at a time, so this round's narration
        // replaces the last round's rather than piling on top of it.
        self.world.resource_mut::<MessageLog>().open_round();
        self.log_kind(MessageKind::Round, format!("── round {round} ──"));
        let player = self.world.resource::<BattleState>().player;
        let plan = self.world.resource::<BattleState>().planned.clone();

        // Bracing is a stance held for the whole round, not an action that
        // only pays off when you win initiative — so it is applied before
        // anyone acts. A defender is therefore already covered against a
        // faster enemy, and already drawing extra fire when targets are
        // rolled.
        for (slot, action) in plan.iter().enumerate() {
            if !matches!(action, Some(BattleAction::Defend)) {
                continue;
            }
            let Some(entity) = self.actor_entity(battle::Actor::Party(slot)) else {
                continue;
            };
            if self.creature_alive(entity) && !self.is_stunned(entity) {
                self.begin_defend(entity);
            }
        }

        for actor in self.roll_initiative() {
            if self.world.get_resource::<BattleState>().is_none() {
                break;
            }
            let Some(entity) = self.actor_entity(actor) else {
                continue;
            };
            // Anything that died earlier this round doesn't get its turn —
            // initiative was rolled before any damage landed.
            if !self.creature_alive(entity) {
                continue;
            }
            if self.is_stunned(entity) {
                let name = self.actor_label(actor, entity);
                self.log(format!("{name} stalls — stunned, and loses the turn!"));
                continue;
            }
            match actor {
                battle::Actor::Party(slot) => {
                    if let Some(Some(action)) = plan.get(slot) {
                        self.resolve_one_action(slot, entity, action.clone(), player);
                    }
                }
                battle::Actor::Enemy { group, .. } => self.wild_retaliate(entity, group, player),
            }
        }

        if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
            battle.round += 1;
            let slots = battle.planned.len();
            battle.planned = vec![None; slots];
        }
        self.tick_round_status_effects(player);
        self.tick();
    }

    /// How to name `actor` in the battle log.
    pub(crate) fn actor_label(&self, actor: battle::Actor, entity: Entity) -> String {
        match actor {
            battle::Actor::Party(0) => "Your process".to_string(),
            _ => self.creature_label(entity),
        }
    }

    /// Executes one party member's chosen action. Every `BattleAction`
    /// variant is handled here — this match is the one place a new action
    /// needs an arm.
    pub(crate) fn resolve_one_action(
        &mut self,
        slot: usize,
        entity: Entity,
        action: BattleAction,
        player: Entity,
    ) {
        match action {
            BattleAction::Attack { group } => {
                let Some(group) = self.retarget(group) else {
                    return;
                };
                self.party_member_attacks(slot, entity, group, player);
            }
            BattleAction::Special { ability, target } => {
                let name = self.creature_label(entity);
                let abilities = self.actor_abilities(entity);
                // Falls back to the first rather than skipping the turn: the
                // index was valid when planned, and a party edited mid-round
                // shouldn't silently cost a member its action.
                let chosen = abilities
                    .get(ability)
                    .or_else(|| abilities.first())
                    .cloned();
                if let Some(ability) = chosen {
                    // Paid before the effect resolves, not after: a killing
                    // blow ends the battle inside `reap_dead_members`, and
                    // `end_battle` wipes every battle-scoped component — so a
                    // cooldown armed afterwards would be written back onto an
                    // entity that has already been cleaned up, and survive
                    // into the next fight.
                    if ability.cooldown > 0 {
                        let mut cooldowns = self
                            .world
                            .get::<AbilityCooldowns>(entity)
                            .map(|c| c.0.clone())
                            .unwrap_or_default();
                        // +1 so the tick at the end of this same round
                        // doesn't eat a round the player never got.
                        cooldowns.insert(ability.id.clone(), ability.cooldown + 1);
                        self.world
                            .entity_mut(entity)
                            .insert(AbilityCooldowns(cooldowns));
                    }
                    // Directing a companion's ability still costs the player
                    // fatigue, as commanding one always did — the action
                    // moved into the round loop, the price didn't go away.
                    if let Some(mut needs) = self.world.get_mut::<Needs>(player) {
                        needs.fatigue = (needs.fatigue - ability.fatigue_cost).max(0.0);
                    }

                    // Decompile needs the *group index*, not the recipient
                    // entity: a successful capture drops the target out of
                    // its group. Every other effect only ever touches the
                    // recipients it lands on.
                    if matches!(ability.effect, AbilityEffect::Decompile) {
                        if let battle::SpecialTarget::EnemyGroup { group } = target
                            && let Some(group) = self.retarget(group)
                        {
                            self.attempt_decompile(group, player);
                        }
                    } else {
                        let recipients = self.ability_recipients(ability.target, &target);
                        self.use_ability(&ability, entity, &name, &recipients);
                        // An area effect can drop members from any rank, and
                        // a corpse left in a group would be promoted to front
                        // and then attacked as though alive.
                        self.reap_dead_members(player);
                    }
                }
            }
            // Already applied up front in `battle_resolve_round`, so that
            // bracing covers the whole round rather than only what happens
            // after this member's place in the initiative order.
            BattleAction::Defend => {}
            BattleAction::UseItem { item } => {
                self.consume_item(&item);
            }
        }
    }

    /// One party member's attack on `group`'s front. The player (slot 0)
    /// has no `Creature` component and so no moveset, and keeps the flat
    /// strike they always had; a companion rolls from its species moves the
    /// way `wild_retaliate` does. Returns whether that ended the battle.
    pub(crate) fn party_member_attacks(
        &mut self,
        slot: usize,
        entity: Entity,
        group: usize,
        player: Entity,
    ) -> bool {
        let Some(front) = self.front_of_group(group) else {
            return false;
        };
        let (move_name, move_power) = if slot == 0 {
            ("data strike".to_string(), PLAYER_STRIKE_POWER)
        } else {
            match self.roll_species_move(entity) {
                Some(mv) => (mv.name.clone(), mv.power),
                None => ("a raw signal burst".to_string(), PLAYER_STRIKE_POWER),
            }
        };
        let (atk, def) = (
            self.effective_atk(entity),
            self.world.get::<Stats>(front).unwrap().def,
        );
        let dmg = battle::compute_damage(atk, def, move_power);
        self.apply_damage(front, dmg);
        if slot == 0 {
            self.log_kind(
                MessageKind::PartyDamage,
                format!("You unleash a {move_name} for {dmg} damage."),
            );
        } else {
            let name = self.creature_label(entity);
            self.log_kind(
                MessageKind::PartyDamage,
                format!("{name} executes {move_name} for {dmg} damage."),
            );
        }

        if !self.creature_alive(front) {
            return self.finish_group_member(group, player);
        }
        false
    }

    /// A uniformly-random move from `entity`'s species moveset, or `None`
    /// if it has no `Creature` component or an empty moveset.
    pub(crate) fn roll_species_move(&mut self, entity: Entity) -> Option<MoveDef> {
        let species_id = self.world.get::<Creature>(entity)?.species.clone();
        let moves = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.moves.clone())?;
        if moves.is_empty() {
            return None;
        }
        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..moves.len())
        };
        Some(moves[idx].clone())
    }

    /// How a planned action reads on the roster. Takes the acting entity so
    /// a planned Special names the ability it will use rather than the
    /// generic word — which of several a member is about to spend is the
    /// part worth reading back.
    pub(crate) fn action_label(&self, actor: Entity, action: &BattleAction) -> String {
        let group_letter = |group: usize| (b'A' + group as u8) as char;
        match action {
            BattleAction::Attack { group } => format!("Attack {}", group_letter(*group)),
            BattleAction::Special { ability, target } => {
                let abilities = self.actor_abilities(actor);
                let name = abilities
                    .get(*ability)
                    .or_else(|| abilities.first())
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "Special".to_string());
                let on = match target {
                    battle::SpecialTarget::EnemyGroup { group } => group_letter(*group).to_string(),
                    battle::SpecialTarget::Ally { slot } => self
                        .actor_entity(battle::Actor::Party(*slot))
                        .map(|e| {
                            if *slot == 0 {
                                "you".to_string()
                            } else {
                                self.creature_label(e)
                            }
                        })
                        .unwrap_or_else(|| "?".to_string()),
                    battle::SpecialTarget::WholeParty => "the party".to_string(),
                    battle::SpecialTarget::AllEnemies => "all groups".to_string(),
                };
                format!("{name} -> {on}")
            }
            BattleAction::Defend => "Defend".to_string(),
            BattleAction::UseItem { item } => format!("Use {}", self.item_name(item)),
        }
    }

    pub fn battle_view(&self) -> Option<BattleView> {
        let battle = self.world.get_resource::<BattleState>()?;
        let decompiler_skill = self.player_decompiler_skill();
        let catalyst_potency = self.taming_catalyst().map(|(_, potency)| potency);

        let groups: Vec<EnemyGroupView> = battle
            .groups
            .iter()
            .enumerate()
            .filter_map(|(idx, group)| {
                let front = group.front()?;
                let stats = self.world.get::<Stats>(front)?;
                let species = self
                    .world
                    .get::<Creature>(front)
                    .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species));
                let species_name = species
                    .map(|s| self.zone_tagged_name(front, s.name.clone()))
                    .unwrap_or_default();
                let taming_difficulty = species
                    .map(|s| s.taming_difficulty)
                    .unwrap_or(DEFAULT_TAMING_DIFFICULTY);
                Some(EnemyGroupView {
                    letter: (b'A' + idx as u8) as char,
                    species_name,
                    count: group.members.len(),
                    front_hp: stats.hp,
                    front_max_hp: stats.max_hp,
                    atk: stats.atk,
                    def: stats.def,
                    is_boss: species.is_some_and(|s| s.is_boss),
                    engaged: idx < ENGAGED_GROUPS,
                    status_effect: self.status_label(front),
                    decompile_chance: catalyst_potency.map(|potency| {
                        taming::capture_chance(
                            stats.hp_fraction(),
                            potency,
                            taming_difficulty,
                            decompiler_skill,
                        )
                    }),
                })
            })
            .collect();

        let party: Vec<PartySlotView> = (0..battle.planned.len())
            .filter_map(|slot| {
                let entity = self.actor_entity(battle::Actor::Party(slot))?;
                let stats = self.world.get::<Stats>(entity)?;
                Some(PartySlotView {
                    slot,
                    entity,
                    name: if slot == 0 {
                        "You".to_string()
                    } else {
                        self.creature_label(entity)
                    },
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    atk: self.effective_atk(entity),
                    def: self.effective_def(entity),
                    status_effect: self.status_label(entity),
                    planned: battle.planned[slot]
                        .as_ref()
                        .map(|action| self.action_label(entity, action)),
                    front: slot < FRONT_SLOTS,
                })
            })
            .collect();

        let active_slot = self.battle_active_slot();
        Some(BattleView {
            groups,
            party,
            active_slot,
            options: active_slot
                .map(|slot| self.battle_action_options(slot))
                .unwrap_or_default(),
            round: battle.round,
            player_decompiler: decompiler_skill,
        })
    }

    /// The front `battle::attackers_in_group` members of each reachable
    /// group retaliate this round — enough of a pack to make it more
    /// dangerous than a solo encounter, without a hundred-strong swarm
    /// simply deleting the party. Each one independently rolls its own move
    /// and target (see `wild_retaliate`).
    pub(crate) fn all_wild_retaliate(&mut self, player: Entity) {
        // Ordered by the same initiative roll the full round loop uses, so
        // a fast pack member lands its hit before a slow one — the party
        // side joins this order in `battle_resolve_round`.
        for actor in self.roll_initiative() {
            let battle::Actor::Enemy { group, .. } = actor else {
                continue;
            };
            let Some(wild) = self.actor_entity(actor) else {
                continue;
            };
            if self.creature_alive(wild) {
                self.wild_retaliate(wild, group, player);
            }
        }
    }

    /// Drops `group`'s member at `index` (the caller is responsible for
    /// whatever happened to it — a kill or a successful tame), removing the
    /// group entirely if that emptied it. Returns whether the whole pack is
    /// gone.
    pub(crate) fn remove_member(&mut self, group: usize, index: usize) -> bool {
        let mut battle = self.world.resource_mut::<BattleState>();
        let Some(g) = battle.groups.get_mut(group) else {
            return battle.groups.is_empty();
        };
        if index < g.members.len() {
            g.members.remove(index);
        }
        if g.members.is_empty() {
            battle.groups.remove(group);
        }
        battle.groups.is_empty()
    }

    /// Handles `group`'s member at `index` dying (from a direct hit, an area
    /// effect, or a status tick): logs the kill, awards its loot/XP,
    /// despawns it, and drops it from the group. If that emptied the last
    /// standing group, the whole encounter ends in a win (`BattleState`
    /// removed) and this returns `true`; otherwise the fight continues,
    /// returning `false`.
    pub(crate) fn finish_member(&mut self, group: usize, index: usize, player: Entity) -> bool {
        let Some(victim) = self
            .world
            .get_resource::<BattleState>()
            .and_then(|b| b.groups.get(group))
            .and_then(|g| g.members.get(index))
            .copied()
        else {
            return self.living_group_count() == 0;
        };
        self.log_kind(
            MessageKind::Outcome,
            "The rogue program crashes and deletes itself!",
        );
        let wild_max_hp = self.world.get::<Stats>(victim).unwrap().max_hp;
        self.award_player_xp(player, wild_max_hp as u32);
        self.award_loot(victim);
        let nest = self.world.get::<NestGuardian>(victim).map(|g| g.nest);
        self.world.despawn(victim);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        if self.remove_member(group, index) {
            self.end_battle(player, Some(victim));
            true
        } else {
            // Only a front kill promotes someone into the line of fire; a
            // back-rank death changes nothing the player can see.
            if index == 0 {
                self.log("Another rogue program from the pack engages!");
            }
            false
        }
    }

    pub(crate) fn finish_group_member(&mut self, group: usize, player: Entity) -> bool {
        self.finish_member(group, 0, player)
    }

    /// How an entity reads as the *object* of a log line — "you" for the
    /// player, its own name otherwise. `creature_label` is the subject form
    /// and returns "You" for the player, which reads wrong mid-sentence.
    pub(crate) fn target_label(&self, entity: Entity) -> String {
        if entity == self.player_entity() {
            "you".to_string()
        } else {
            self.creature_label(entity)
        }
    }

    /// Which entities an ability actually lands on this round, resolved at
    /// resolve time rather than plan time — so a group that died before the
    /// acting member's turn retargets, and an ally knocked out in the
    /// meantime is skipped instead of being healed as a corpse.
    pub(crate) fn ability_recipients(
        &self,
        target: AbilityTarget,
        chosen: &battle::SpecialTarget,
    ) -> Vec<Entity> {
        match target {
            AbilityTarget::OneAlly => match chosen {
                battle::SpecialTarget::Ally { slot } => self
                    .actor_entity(battle::Actor::Party(*slot))
                    .filter(|&e| self.creature_alive(e))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeParty => (0..self
                .world
                .get_resource::<BattleState>()
                .map(|b| b.planned.len())
                .unwrap_or(0))
                .filter_map(|slot| self.actor_entity(battle::Actor::Party(slot)))
                .filter(|&e| self.creature_alive(e))
                .collect(),
            AbilityTarget::OneEnemyGroupFront => match chosen {
                battle::SpecialTarget::EnemyGroup { group } => self
                    .retarget(*group)
                    .and_then(|g| self.front_of_group(g))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeEnemyGroup => match chosen {
                battle::SpecialTarget::EnemyGroup { group } => self
                    .retarget(*group)
                    .and_then(|g| {
                        self.world
                            .get_resource::<BattleState>()
                            .and_then(|b| b.groups.get(g))
                            .map(|grp| grp.members.clone())
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|&e| self.creature_alive(e))
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::AllEnemies => self.all_living_enemies(),
        }
    }

    /// Executes `ability` (one of `Game::actor_abilities`) on every
    /// entity in `recipients` — party members for a buff or heal, enemies
    /// for damage or a debuff. See `Game::ability_recipients`, which
    /// resolves which entities those are. `actor` is who is spending the
    /// ability, which a damage effect needs for its ATK.
    pub(crate) fn use_ability(
        &mut self,
        ability: &AbilityDef,
        actor: Entity,
        name: &str,
        recipients: &[Entity],
    ) {
        for &recipient in recipients {
            // A buff can land on the player or on a companion, so the log
            // names whoever got it rather than assuming "you".
            let on = self.target_label(recipient);
            match &ability.effect {
                AbilityEffect::Buff {
                    kind,
                    power,
                    duration,
                } => {
                    self.arm_buff(
                        recipient,
                        ActiveBuff {
                            kind: *kind,
                            remaining: *duration,
                            power: *power,
                        },
                    );
                    let stat = match kind {
                        BuffKind::Atk => "attack",
                        BuffKind::Def => "defense",
                    };
                    self.log(format!(
                        "{name} runs {} on {on}, boosting {stat}!",
                        ability.name
                    ));
                }
                AbilityEffect::Heal { power } => {
                    if let Some(mut stats) = self.world.get_mut::<Stats>(recipient) {
                        stats.hp = (stats.hp + power).min(stats.max_hp);
                    }
                    self.log(format!("{name} patches {on} for {power} HP."));
                }
                AbilityEffect::Debuff {
                    kind,
                    power,
                    duration,
                } => {
                    if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(recipient) {
                        statuses.active = Some(ActiveStatus {
                            kind: *kind,
                            remaining: *duration,
                            power: *power,
                        });
                    }
                    match kind {
                        StatusKind::Bleed => self.log(format!("{name} corrupts {on}'s data!")),
                        StatusKind::Stun => self.log(format!("{name} locks up {on}!")),
                    }
                }
                AbilityEffect::Damage { power, status } => {
                    let def = self
                        .world
                        .get::<Stats>(recipient)
                        .map(|s| s.def)
                        .unwrap_or(0);
                    let dmg = battle::compute_damage(self.effective_atk(actor), def, *power);
                    self.apply_damage(recipient, dmg);
                    self.log_kind(
                        MessageKind::PartyDamage,
                        format!("{name} hits {on} for {dmg} damage."),
                    );
                    if let Some(effect) = status.clone() {
                        self.apply_status_effect(recipient, &effect, &on, MessageKind::PartyDamage);
                    }
                }
                // `resolve_one_action` branches around `use_ability` entirely
                // for `Decompile` — it needs the group index, not a
                // recipient entity — so this arm is unreachable in practice.
                AbilityEffect::Decompile => unreachable!(
                    "Decompile never reaches use_ability; resolve_one_action handles it directly"
                ),
            }
        }
    }

    /// `entity`'s effective ATK for damage purposes: its real `Stats`
    /// value, plus an active `CombatBuff::Atk` bonus if any. If `entity` is
    /// the player, this also adds the standing party bonus (see
    /// `party_stat_bonus`) and applies the low-power attack penalty (see
    /// `battle::power_attack_multiplier`) — both are player-only effects.
    /// `entity` isn't always the player: `wild_retaliate` can call this
    /// (via `effective_def`) with a companion that's eating the hit
    /// instead, and a companion has neither a `Party` bonus of its own nor
    /// `Needs` to run low on.
    pub(crate) fn effective_atk(&self, entity: Entity) -> i32 {
        let base = self.world.get::<Stats>(entity).map(|s| s.atk).unwrap_or(0);
        let bonus = self
            .world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Atk)
            .map(|a| a.power)
            .unwrap_or(0);
        if entity != self.player_entity() {
            return base + bonus;
        }
        let total = base + bonus + self.party_stat_bonus().0;
        let hunger = self
            .world
            .get::<Needs>(entity)
            .map(|n| n.hunger)
            .unwrap_or(100.0);
        ((total as f32) * battle::power_attack_multiplier(hunger)).round() as i32
    }

    /// `entity`'s effective DEF against incoming damage: its real `Stats`
    /// value, plus an active `CombatBuff::Def` bonus if any, plus the
    /// standing party bonus (see `party_stat_bonus`) if `entity` is the
    /// player. Same non-player-safe behavior as `effective_atk`.
    pub(crate) fn effective_def(&self, entity: Entity) -> i32 {
        let base = self.world.get::<Stats>(entity).map(|s| s.def).unwrap_or(0);
        let bonus = self
            .world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Def)
            .map(|a| a.power)
            .unwrap_or(0);
        if entity != self.player_entity() {
            return base + bonus;
        }
        base + bonus + self.party_stat_bonus().1
    }

    /// Standing `(atk, def)` bonus the player gets just for having programs
    /// in their active party — each member contributes 10% of its own
    /// current ATK and DEF (minimum 1 each), summed across the party.
    /// Computed live from each companion's current `Stats` rather than
    /// baked into the player's own `Stats` on add/remove, so it stays
    /// correct automatically as a companion levels up, is fused, or dies —
    /// no separate bookkeeping to keep in sync.
    pub(crate) fn party_stat_bonus(&self) -> (i32, i32) {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.world.get::<Stats>(e))
            .fold((0, 0), |(atk, def), s| {
                (
                    atk + (s.atk / PARTY_PASSIVE_STAT_DIVISOR).max(1),
                    def + (s.def / PARTY_PASSIVE_STAT_DIVISOR).max(1),
                )
            })
    }
}
