//! Resolving a planned round — running each actor's action, computing
//! effective stats, and rendering the result as a `BattleView`.

use crate::abilities::PassiveTrigger;
use crate::tuning::{
    ENGAGED_GROUPS, FRONT_SLOTS, MAX_MITIGATION_PERCENT, NEST_RESPAWN_TICKS,
    PARTY_PASSIVE_STAT_DIVISOR, PLAYER_STRIKE_POWER, WIELDED_PROGRAM_STAT_DIVISOR,
    WIELDED_ROUTINE_PROC_CHANCE,
};
use crate::*;

impl Game {
    /// Resolves a *planned* group index against the groups as they stand
    /// now, returning where that group is currently indexed — or `None` if
    /// it has fallen, in which case the action is spent rather than
    /// redirected.
    ///
    /// A planned index names a group, not a slot. An emptied group is
    /// dropped from `BattleState::groups` the moment it dies, shifting
    /// every group behind it down one, so an index alone stops meaning what
    /// the player picked: aim at group B, watch A fall to a faster party
    /// member, and the raw index would land on C. Matching on
    /// `BattleState::round_targets` — the member sets captured when the
    /// plan was made — is what makes the aim follow the group instead.
    ///
    /// The turn is deliberately wasted when the target is gone. Falling
    /// back to the front group, which is what this used to do, is the
    /// overflow itself: it spends a heavy hit or a decompile on something
    /// nobody aimed at.
    pub(crate) fn retarget(&self, group: usize) -> Option<usize> {
        let battle = self.world.get_resource::<BattleState>()?;
        let planned = battle.round_targets.get(group)?;
        battle
            .groups
            .iter()
            .position(|g| g.members.iter().any(|m| planned.contains(m)))
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
        // At the top of the round, before anything resolves. Taken at the
        // end it would be a snapshot of the aftermath, and the round-by-round
        // curve the file is for would be shifted by one against the actions
        // that produced it.
        let fight = self.fight_id();
        self.record(|g| crate::telemetry::Record::Round {
            fight,
            round,
            party_hp: g.telemetry_party_hp(),
            enemies: g.telemetry_enemy_hp(),
        });
        // The pane shows one round at a time, so this round's narration
        // replaces the last round's rather than piling on top of it — and
        // the frames pacing that narration are scoped to the same range.
        self.world.resource_mut::<MessageLog>().open_round();
        self.world.resource_mut::<BattleTimeline>().frames.clear();
        // A frame at zero lines, before the header goes out. `App::
        // revealed_count` reports zero for the whole gap between a round
        // resolving and the next frame the frontend draws, so without this
        // the roster would fall back to live rows and flash the finished
        // round for one frame before starting to scroll.
        self.snapshot_roster();
        self.log_kind(MessageKind::Round, format!("── round {round} ──"));
        let player = self.world.resource::<BattleState>().player;
        let plan = self.world.resource::<BattleState>().planned.clone();
        // Captured alongside the plan, and for the plan's sake: the indices
        // in it are only meaningful against the groups as they stood when
        // it was made. See `BattleState::round_targets`.
        let mut battle = self.world.resource_mut::<BattleState>();
        battle.round_targets = battle.groups.iter().map(|g| g.members.clone()).collect();

        // Under the round header, so the narration reads inside the round it
        // opens, and ahead of every chosen action, because that is the whole
        // of what this trigger means. After `round_targets`, not before: an
        // enemy-facing passive resolves its group through `Game::retarget`,
        // which reads it, and last round's snapshot would aim at a group that
        // may not be standing any more.
        //
        // The whole living party rather than one combatant, the way
        // `AllyDropped` does — a round starting is a fact about the round.
        let living = self.living_party();
        self.fire_passives(PassiveTrigger::RoundStart, &living);
        if self.world.get_resource::<BattleState>().is_none() {
            return;
        }

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

        // Taken before anyone acts, so `AllyDropped` can be answered by
        // comparing against it afterwards rather than by threading a flag
        // through every path that can drop a party member —
        // `reap_dead_members`, a status tick, a hostile's routine and a
        // friendly-fire sweep all reach the same end state, and only one of
        // them would have remembered to set the flag.
        let party_before = self.living_party();
        // The same snapshot again, with Integrity, because `AllyWounded`
        // asks whether a member *crossed* the line rather than whether they
        // are under it — a member sitting at 20% for six rounds is one
        // crisis, not six.
        let integrity_before = self.party_integrity();

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

        // Passives, after every chosen action and before the round is
        // closed out. All three triggers are answered from state rather
        // than from an event stream, which is what lets them sit here in
        // one place instead of at every site that could cause them.
        //
        // `Afflicted` is read *before* `tick_round_status_effects`, which
        // clears `landed_this_round` — see `Game::newly_afflicted_party`.
        //
        // `AllyWounded` before `AllyDropped` and not after: a round that
        // takes a member from healthy to dead is the second event, not both,
        // and `newly_wounded_party` keeps them disjoint by only reporting
        // survivors. Ordering them this way means a member wounded by the
        // same round that killed a *different* one still gets their answer.
        if self.world.get_resource::<BattleState>().is_some() {
            let wounded = self.newly_wounded_party(&integrity_before);
            if !wounded.is_empty() {
                self.fire_passives(PassiveTrigger::AllyWounded, &wounded);
            }
            if party_before.iter().any(|&e| !self.creature_alive(e)) {
                let living = self.living_party();
                self.fire_passives(PassiveTrigger::AllyDropped, &living);
            }
            let afflicted = self.newly_afflicted_party();
            if !afflicted.is_empty() {
                self.fire_passives(PassiveTrigger::Afflicted, &afflicted);
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

    /// Puts `ability` on cooldown for `entity`, if it has one to arm.
    ///
    /// **Always call this before the effect resolves.** A killing blow ends
    /// the battle inside `reap_dead_members`, and `end_battle` wipes every
    /// battle-scoped component — so a cooldown armed afterwards is written
    /// back onto an entity that has already been cleaned up, and survives
    /// into the next fight.
    ///
    /// `floor = 0`: the player side keeps the authored value untouched (see
    /// `abilities::armed_cooldown`), which is what leaves `decompile` —
    /// guarded out by the `cooldown > 0` check — spammable.
    ///
    /// Shared by `resolve_one_action` and `Game::fire_passives`, which is
    /// the whole reason it is a function: a passive that armed its cooldown
    /// by a second copy of this would drift from the chosen-Special one the
    /// first time either was retuned.
    pub(crate) fn arm_cooldown(&mut self, entity: Entity, ability: &AbilityDef) {
        if ability.cooldown == 0 {
            return;
        }
        let mut cooldowns = self
            .world
            .get::<AbilityCooldowns>(entity)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        cooldowns.insert(
            ability.id.clone(),
            abilities::armed_cooldown(ability.cooldown, 0),
        );
        self.world
            .entity_mut(entity)
            .insert(AbilityCooldowns(cooldowns));
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
        // Before the action resolves: this is a record of what was *chosen*,
        // the party-side counterpart to `enemy_choice`. Reached only for a
        // member that actually acts — `battle_resolve_round` skips the dead
        // and the stunned before calling — so a recorded action is one that
        // happened.
        let fight = self.fight_id();
        let round = self.telemetry_round();
        // `telemetry_action` allocates — a `Vec` of abilities and a `String`
        // — so it is called *inside* the closure, which is the whole reason
        // `record` takes one. Borrowing `action` here is fine: the closure
        // runs to completion inside `record`, before the match below moves it.
        self.record(|g| {
            let (kind, name, target_slot) = g.telemetry_action(entity, &action);
            crate::telemetry::Record::PartyAction {
                fight,
                round,
                slot,
                actor: g.telemetry_actor_label(slot, entity),
                kind,
                name,
                target_slot,
            }
        });
        match action {
            BattleAction::Attack { group } => {
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
                    self.arm_cooldown(entity, &ability);
                    // Charged here rather than in `use_ability`, for the same
                    // reason and at the same moment as the cooldown above.
                    // `use_ability` is also the path `proc_wielded_routine`
                    // and hostile casts take, and both are deliberately free —
                    // the proc's 25% rate is its whole price, and hostiles
                    // hold no reserve at all.
                    //
                    // The *caster* pays: `entity` is whoever is acting, so a
                    // companion's Special draws on the companion's reserve.
                    // Directing one used to come out of the player's meter,
                    // which rationed the party's own kit against a pool only
                    // the player had.
                    self.spend_power(entity, abilities::routine_power_cost(&ability));

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
                        let recipients = self.ability_recipients(entity, ability.target, &target);
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
    ///
    /// Two orderings in the tail are load-bearing. The wielded-program proc
    /// comes *after* the front's death is resolved, so a routine can never
    /// land on a corpse and never fires at all once `finish_group_member`
    /// has ended the battle. And it comes before the return, so its own
    /// kills are reaped inside `proc_wielded_routine` the way
    /// `resolve_one_action`'s `Special` arm reaps its own.
    pub(crate) fn party_member_attacks(
        &mut self,
        slot: usize,
        entity: Entity,
        group: usize,
        player: Entity,
    ) -> bool {
        // `group` is the index the *plan* named, so it is resolved here and
        // again for the proc below rather than being resolved once by the
        // caller: a strike that empties its target re-letters the groups
        // behind it, and the proc has to be answering the same aim, not the
        // shifted one.
        let Some(live) = self.retarget(group) else {
            return false;
        };
        let Some(front) = self.front_of_group(live) else {
            return false;
        };
        let (move_name, natural) = if slot == 0 {
            (
                "data strike".to_string(),
                battle::DamageRange::centred(PLAYER_STRIKE_POWER, 0),
            )
        } else {
            match self.roll_species_move(entity) {
                Some(mv) => (mv.name.clone(), mv.attack_parts().0),
                None => (
                    "a raw signal burst".to_string(),
                    battle::DamageRange::centred(PLAYER_STRIKE_POWER, 0),
                ),
            }
        };
        // Task 8 turns this into a roll through `attack_range`; the mean of a
        // centred band is exactly the flat power this read before.
        let move_power = natural.mean().round() as i32;
        // No mitigation term here: `apply_damage` owns that, as the
        // percentage cut it now is. Passing it to `compute_damage` as well
        // would subtract it once and then cut by it again.
        let raw = battle::compute_damage(self.effective_atk(entity), 0, move_power);
        let dmg = self.apply_damage(front, raw);
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

        if !self.creature_alive(front) && self.finish_group_member(live, player) {
            return true;
        }
        if slot == 0 {
            return self.proc_wielded_routine(group, player);
        }
        false
    }

    /// Rolls the wielded program's chance to fire one of its own routines on
    /// top of the player's strike, and resolves it. Returns whether that
    /// ended the battle.
    ///
    /// **The actor is the program, not the player.** `use_ability` reads
    /// `ability_user_level`, `ability_affinity` and `effective_atk` off
    /// whoever it is handed, so passing the program is what makes *which*
    /// program you wield decide what a proc is worth — the whole point of
    /// the feature. A tamed program carries no `Hostile`, so
    /// `ability_recipients` takes the friendly branch for free.
    ///
    /// Costs nothing: no Power, no cooldown armed. The program is not in
    /// the battle line and has no `AbilityCooldowns` tick of its own to hang
    /// one off, and inventing bookkeeping for a non-combatant buys nothing
    /// at this rate. Nothing happens to the program either — no damage, no
    /// XP, no status. It is a weapon, not a combatant.
    fn proc_wielded_routine(&mut self, group: usize, player: Entity) -> bool {
        let Some(program) = self.wielded_program() else {
            return false;
        };
        // Both resolved before the roll, so a program with nothing legal to
        // fire consumes no `GameRng` and can't shift every later draw in the
        // run just by being held.
        let routines = self.wieldable_routines(program);
        if routines.is_empty() {
            return false;
        }
        // The strike may have just emptied the group it targeted.
        let Some(group) = self.retarget(group) else {
            return false;
        };
        let Some(ability) = ({
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool(WIELDED_ROUTINE_PROC_CHANCE)
                .then(|| rng.0.random_range(0..routines.len()))
        })
        .map(|i| routines[i].clone()) else {
            return false;
        };
        // A proc opens no picker, so its target is synthesized from the
        // attack that triggered it. `OneAlly` resolves to the player rather
        // than a companion deliberately: it is *your* weapon, and slot 0 is
        // the one ally guaranteed to exist.
        let target = match ability.target {
            AbilityTarget::OneEnemyGroupFront | AbilityTarget::WholeEnemyGroup => {
                battle::SpecialTarget::EnemyGroup { group }
            }
            AbilityTarget::OneAlly => battle::SpecialTarget::Ally { slot: 0 },
            AbilityTarget::WholeParty => battle::SpecialTarget::WholeParty,
            AbilityTarget::AllEnemies => battle::SpecialTarget::AllEnemies,
        };
        let name = self.creature_label(program);
        let recipients = self.ability_recipients(program, ability.target, &target);
        self.use_ability(&ability, program, &name, &recipients);
        self.reap_dead_members(player)
    }

    /// A uniformly-random basic attack from `entity`'s species, or `None`
    /// if it has no `Creature` component or no attacks at all.
    pub(crate) fn roll_species_move(&mut self, entity: Entity) -> Option<AbilityDef> {
        let species_id = self.world.get::<Creature>(entity)?.species.clone();
        let moves = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.basic_attacks())?;
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

    /// The live roster — every row of both sides as things stand right now.
    ///
    /// Split out of `battle_view` so `snapshot_roster` records exactly what
    /// the screen would have drawn, rather than a second, drifting notion of
    /// what a row holds.
    pub(crate) fn battle_rows(&self) -> Option<(Vec<EnemyGroupView>, Vec<PartySlotView>)> {
        let battle = self.world.get_resource::<BattleState>()?;
        let bonuses = self.player_decompiler_bonuses();
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
                let resistance = self.target_resistance(front)?;
                let is_boss = species.is_some_and(|s| s.is_boss);
                Some(EnemyGroupView {
                    letter: (b'A' + idx as u8) as char,
                    species_name,
                    count: group.members.len(),
                    front_hp: stats.hp,
                    front_max_hp: stats.max_hp,
                    front_rarity: self.rarity_of(front),
                    atk: stats.atk,
                    def: stats.mitigation,
                    is_boss,
                    engaged: idx < ENGAGED_GROUPS,
                    status_effect: self.status_label(front),
                    // No odds against a boss, because there is no attempt to
                    // make — `battle_set_action` refuses the target outright.
                    decompile_chance: catalyst_potency
                        .filter(|_| !is_boss)
                        .map(|potency| taming::capture_chance(potency, resistance, bonuses)),
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
                    def: self.effective_mitigation(entity),
                    status_effect: self.status_label(entity),
                    power: self.world.get::<PowerReserve>(entity).map(|n| n.get()),
                    planned: battle.planned[slot]
                        .as_ref()
                        .map(|action| self.action_label(entity, action)),
                    front: slot < FRONT_SLOTS,
                    gear: self.gear_tag(entity),
                })
            })
            .collect();

        Some((groups, party))
    }

    /// The battle screen's whole readout, as things stand right now. This is
    /// the truthful view: input handling maps a typed group letter through
    /// it, and every test reads it. A renderer pacing narration wants
    /// `battle_view_at` instead.
    pub fn battle_view(&self) -> Option<BattleView> {
        let (groups, party) = self.battle_rows()?;
        self.assemble_view(groups, party)
    }

    /// The battle screen's readout as of `revealed` narrated lines of the
    /// current round — the roster stepping in time with the log rather than
    /// snapping to the end of the round before line one is legible.
    ///
    /// Falls back to the live rows when no frame covers that far back, which
    /// covers a fight whose reveal has already caught up and a loaded game
    /// whose timeline is empty.
    pub fn battle_view_at(&self, revealed: usize) -> Option<BattleView> {
        let (groups, party) = match self.world.resource::<BattleTimeline>().frame_at(revealed) {
            Some(frame) => (frame.groups.clone(), frame.party.clone()),
            None => self.battle_rows()?,
        };
        self.assemble_view(groups, party)
    }

    /// Wraps a pair of roster halves in the rest of the screen's state.
    ///
    /// `active_slot` and its `options` are deliberately live rather than
    /// recorded: they are what the *next* keypress will do, not part of the
    /// round being narrated, and the action bar they drive is hidden while
    /// narration is still scrolling in anyway.
    fn assemble_view(
        &self,
        groups: Vec<EnemyGroupView>,
        party: Vec<PartySlotView>,
    ) -> Option<BattleView> {
        let battle = self.world.get_resource::<BattleState>()?;
        let active_slot = self.battle_active_slot();
        Some(BattleView {
            groups,
            party,
            active_slot,
            options: active_slot
                .map(|slot| self.battle_action_options(slot))
                .unwrap_or_default(),
            round: battle.round,
            player_decompiler: self.player_decompiler_bonuses().skill,
        })
    }

    /// Records the roster as it stands into the current round's timeline,
    /// tagged with the log length that made it. Called after every battle
    /// log line — see `BattleTimeline`.
    pub(crate) fn snapshot_roster(&mut self) {
        let Some((groups, party)) = self.battle_rows() else {
            return;
        };
        let lines = self.battle_log().len();
        self.world
            .resource_mut::<BattleTimeline>()
            .frames
            .push(RosterFrame {
                lines,
                groups,
                party,
            });
    }

    /// The battle screen as the last fight ended — what it keeps drawing
    /// while the results scroll into its log pane, once `end_battle` has
    /// removed `BattleState` and `battle_view` has gone `None`.
    ///
    /// No `active_slot` and no `options`: nothing is choosing anything, and
    /// the caller draws a continue prompt where the action bar was. See
    /// `BattleTimeline::closing`.
    pub fn battle_result_view(&self) -> Option<BattleView> {
        let closing = self.world.resource::<BattleTimeline>().closing.as_ref()?;
        Some(BattleView {
            groups: closing.groups.clone(),
            party: closing.party.clone(),
            active_slot: None,
            options: Vec::new(),
            round: closing.round,
            player_decompiler: closing.player_decompiler,
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
        let earned = self.kill_xp(victim);
        self.award_player_xp(player, earned);
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

    /// Whether `entity` is fighting on the wild side. Ability targets are
    /// authored from the party's point of view, so this is what decides
    /// which way to read them — see `ability_recipients`.
    pub(crate) fn is_hostile(&self, entity: Entity) -> bool {
        self.world.get::<Hostile>(entity).is_some()
    }

    /// The player plus every living companion — the party side as a flat
    /// list. What a hostile's enemy-facing ability lands on.
    pub(crate) fn living_party(&self) -> Vec<Entity> {
        let battle_slots = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        (0..battle_slots)
            .filter_map(|slot| self.actor_entity(battle::Actor::Party(slot)))
            .filter(|&e| self.creature_alive(e))
            .collect()
    }

    /// The inverse of `actor_entity(battle::Actor::Party(slot))`: which party
    /// slot `entity` occupies, if it occupies one at all. `roll_enemy_target`
    /// returns an entity; a hostile's single-target routine needs it as a
    /// slot index to build `SpecialTarget::Ally` for `ability_recipients`.
    pub(crate) fn party_slot_of(&self, entity: Entity) -> Option<usize> {
        if entity == self.player_entity() {
            return Some(0);
        }
        self.world
            .resource::<Party>()
            .0
            .iter()
            .position(|&e| e == entity)
            .map(|i| i + 1)
    }

    /// Which entities `target` lands on, read from `actor`'s side of the
    /// fight.
    ///
    /// Resolved at resolve time rather than plan time — so a group that died
    /// before the acting member's turn retargets, and an ally knocked out in
    /// the meantime is skipped instead of being healed as a corpse.
    ///
    /// Targets are authored from the party's point of view — "ally" means a
    /// party member, "enemy" means a wild program. A hostile using the same
    /// ability flips both: its ally is another hostile, and its enemy is the
    /// party. That mirror is what lets one ability file serve both sides
    /// instead of needing an enemy-only twin.
    ///
    /// Two of the shapes collapse on the hostile side. The party is a single
    /// flat roster where the wild side is partitioned into groups, so
    /// `WholeEnemyGroup` has no player-side subdivision to select and reads
    /// identically to `AllEnemies`. That is the asymmetry of the two sides,
    /// not a shortcut.
    pub(crate) fn ability_recipients(
        &self,
        actor: Entity,
        target: AbilityTarget,
        chosen: &battle::SpecialTarget,
    ) -> Vec<Entity> {
        if self.is_hostile(actor) {
            return match target {
                AbilityTarget::OneAlly => self.hostile_ally_of(actor).into_iter().collect(),
                AbilityTarget::WholeParty => self.all_living_enemies(),
                AbilityTarget::OneEnemyGroupFront => match chosen {
                    battle::SpecialTarget::Ally { slot } => self
                        .actor_entity(battle::Actor::Party(*slot))
                        .filter(|&e| self.creature_alive(e))
                        .into_iter()
                        .collect(),
                    // Nothing else constructs a `SpecialTarget` for a
                    // hostile's single-target routine: `wild_retaliate`
                    // always rolls `roll_enemy_target` first and passes the
                    // result as `Ally`. Empty rather than
                    // `living_party().take(1)` — that fallback used to be
                    // the only live path, and it silently hit the player
                    // (slot 0) every time, bypassing bracing and aggro
                    // weighting entirely.
                    _ => Vec::new(),
                },
                AbilityTarget::WholeEnemyGroup | AbilityTarget::AllEnemies => self.living_party(),
            };
        }
        match target {
            AbilityTarget::OneAlly => match chosen {
                battle::SpecialTarget::Ally { slot } => self
                    .actor_entity(battle::Actor::Party(*slot))
                    .filter(|&e| self.creature_alive(e))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeParty => self.living_party(),
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

    /// One living hostile for a carrier's ally-facing routine to land on.
    ///
    /// Not "the most hurt". A carrier fires whenever its routine is off
    /// cooldown, so a heal landing on a healthy ally is wasted — accepted,
    /// because the alternative is a per-effect situational policy that this
    /// design deliberately does not have.
    ///
    /// A deterministic rotation keyed on the round number and the actor's
    /// own position among the candidates — not a uniform draw. Chosen so the
    /// pick needs no `&mut self` (and so no `GameRng`, which only takes
    /// `&mut`), at the cost of being predictable to anyone tracking the
    /// round count. Nothing in the design relies on it being anything else.
    fn hostile_ally_of(&self, actor: Entity) -> Option<Entity> {
        let candidates = self.all_living_enemies();
        if candidates.is_empty() {
            return None;
        }
        let round = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.round as usize)
            .unwrap_or(0);
        let offset = candidates.iter().position(|&e| e == actor).unwrap_or(0);
        Some(candidates[(round + offset) % candidates.len()])
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
        // Both resolved once for the whole cast rather than per recipient:
        // every recipient of one ability is scaled by the *user's* level and
        // affinity, and re-reading either inside the loop would invite
        // someone to key it off the recipient instead.
        let level = self.ability_user_level(actor);
        let affinity = self.ability_affinity(actor, &ability.effect);
        // Damage/drain lines get the log kind their side actually earns,
        // rather than the party's own `PartyDamage` regardless of who is
        // acting — `use_ability` serves both sides now, but a hostile
        // carrier's hit is not a party hit. Derived from `actor` rather than
        // taken as a parameter, since a parameter would be a second source
        // of truth for something `ability_recipients` already determines
        // from `actor` via `is_hostile`.
        let hostile = self.is_hostile(actor);
        let hit_kind = if hostile {
            MessageKind::EnemySpecial
        } else {
            MessageKind::PartyDamage
        };
        // Same split for the lines that restore Integrity — a patch and a
        // drain alike — and for the same reason: a hostile mending its own
        // group, or siphoning off the party, is the party's bad news, so
        // only the party's own restore earns the kind that reads as good.
        let heal_kind = if hostile {
            MessageKind::EnemySpecial
        } else {
            MessageKind::Heal
        };
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
                            power: abilities::scaled_stat_power(*power, level, affinity),
                        },
                    );
                    let stat = match kind {
                        BuffKind::Atk => "attack",
                        BuffKind::Mitigation => "defense",
                    };
                    self.log(format!(
                        "{name} runs {} on {on}, boosting {stat}!",
                        ability.name
                    ));
                }
                AbilityEffect::Heal { power } => {
                    let power = abilities::scaled_hp_power(*power, level, affinity);
                    let restored = self.restore_hp(recipient, power);
                    self.log_kind(heal_kind, format!("{name} patches {on} for {restored} HP."));
                }
                AbilityEffect::Debuff {
                    kind,
                    power,
                    duration,
                } => {
                    self.arm_status(
                        recipient,
                        *kind,
                        *duration,
                        abilities::scaled_hp_power(*power, level, affinity),
                    );
                    match kind {
                        StatusKind::Bleed => self.log(format!("{name} corrupts {on}'s data!")),
                        StatusKind::Stun => self.log(format!("{name} locks up {on}!")),
                    }
                }
                AbilityEffect::Damage {
                    power,
                    spread,
                    status,
                } => {
                    // Mitigation is `apply_damage`'s, not a term here — see
                    // `party_member_attacks` for why passing it to
                    // `compute_damage` too would count it twice.
                    let band = abilities::scaled_range(
                        battle::DamageRange::centred(*power, *spread),
                        level,
                        affinity,
                    );
                    let raw = battle::compute_damage(
                        self.effective_atk(actor),
                        0,
                        band.mean().round() as i32,
                    );
                    let dmg = self.apply_damage(recipient, raw);
                    self.log_kind(hit_kind, format!("{name} hits {on} for {dmg} damage."));
                    if let Some(effect) = status.clone() {
                        self.apply_status_effect(recipient, &effect, &on, hit_kind);
                    }
                }
                AbilityEffect::Drain {
                    power,
                    spread,
                    heal_fraction,
                } => {
                    let band = abilities::scaled_range(
                        battle::DamageRange::centred(*power, *spread),
                        level,
                        affinity,
                    );
                    let raw = battle::compute_damage(
                        self.effective_atk(actor),
                        0,
                        band.mean().round() as i32,
                    );
                    let dmg = self.apply_damage(recipient, raw);
                    // Off the damage actually dealt, not the authored power:
                    // mitigation has already eaten into it, and healing off
                    // the pre-mitigation figure would make a drain better
                    // against an armoured target than a soft one.
                    let siphoned = (dmg as f32 * heal_fraction).round() as i32;
                    let restored = self.restore_hp(actor, siphoned);
                    // `heal_kind`, not `hit_kind`: the Integrity coming back
                    // is the half a plain `Attack` cannot also produce, and
                    // so the half the line is read for. It keeps the number
                    // emphasis `PartyDamage` gave it because `Heal` shares
                    // that styling — see `render/mod.rs::draw_message_line`.
                    self.log_kind(
                        heal_kind,
                        format!("{name} siphons {dmg} from {on}, restoring {restored}."),
                    );
                }
                AbilityEffect::Cleanse => {
                    let had_status = self
                        .world
                        .get::<StatusEffects>(recipient)
                        .is_some_and(|s| s.active.is_some());
                    if had_status {
                        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(recipient) {
                            statuses.active = None;
                        }
                        self.log(format!("{name} flushes the corruption from {on}."));
                    }
                    // Silent on a clean recipient: a "nothing to clear" line
                    // per party member, every cast, would drown the log.
                }
                // `resolve_one_action` branches around `use_ability` entirely
                // for `Decompile` — it needs the group index, not a
                // recipient entity — so this arm is unreachable in practice.
                AbilityEffect::Decompile => unreachable!(
                    "Decompile never reaches use_ability; resolve_one_action handles it directly"
                ),
                // The two paths that pick an ability for `use_ability` to run
                // — `battle_special_options` (player) and `wild_routine_ready`
                // (a carrier's retaliation) — both exclude a field-only
                // effect, since none of the three has anything to resolve
                // against a battle recipient.
                AbilityEffect::FieldBuff { .. } | AbilityEffect::Phase | AbilityEffect::Jump => {
                    unreachable!(
                        "AbilityEffect::field_only; battle_special_options and wild_routine_ready both exclude it"
                    )
                }
            }
        }
    }

    /// `entity`'s effective ATK for damage purposes: its real `Stats`
    /// value, plus an active `CombatBuff::Atk` bonus if any, plus any
    /// running `FieldBuffKind::Atk` power (see `field_buff_power`) — the
    /// two sources are separate components and both apply, summed. If
    /// `entity` is the player, this also adds the standing party bonus (see
    /// `party_stat_bonus`) and applies the low-power attack penalty (see
    /// `battle::power_attack_multiplier`) — both are player-only effects.
    /// `entity` isn't always the player: `wild_retaliate` can call this
    /// (via `effective_mitigation`) with a companion that's eating the hit
    /// instead, and a companion has neither a `Party` bonus of its own nor
    /// `PowerReserve` to run low on.
    pub(crate) fn effective_atk(&self, entity: Entity) -> i32 {
        let base = self.world.get::<Stats>(entity).map(|s| s.atk).unwrap_or(0);
        let bonus = self
            .world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Atk)
            .map(|a| a.power)
            .unwrap_or(0);
        let field_bonus = self.field_buff_power(entity, FieldBuffKind::Atk);
        if entity != self.player_entity() {
            return base + bonus + field_bonus;
        }
        let total =
            base + bonus + field_bonus + self.party_stat_bonus().0 + self.wielded_stat_bonus().0;
        let power = self
            .world
            .get::<PowerReserve>(entity)
            .map(|r| r.get())
            .unwrap_or(POWER_MAX);
        ((total as f32) * battle::power_attack_multiplier(power)).round() as i32
    }

    /// `entity`'s total mitigation in percentage points, capped at
    /// `MAX_MITIGATION_PERCENT` — the one door onto "how much of an incoming
    /// hit does this creature shrug off".
    ///
    /// `Stats::mitigation` already carries **both** the innate value and
    /// whatever gear is worn: `Game::apply_equipment_delta` bakes an
    /// equipped item's bonus straight into `Stats`. Adding `gear_bonus`
    /// again here would double-count every worn piece — the same trap "no
    /// stats operation may run while a gear bonus is sitting in `Stats`"
    /// already names from the other direction. On top of that sit an active
    /// `CombatBuff::Mitigation`, any running `FieldBuffKind::Mitigation`
    /// power (see `field_buff_power`) — separate components, both apply —
    /// and the standing party bonus (see `party_stat_bonus`) if `entity` is
    /// the player. Same non-player-safe behaviour as `effective_atk`.
    ///
    /// **The cap is applied here rather than at the readers**, so nothing
    /// downstream can see an uncapped percentage and none of them needs to
    /// remember to clamp.
    ///
    /// `is_defending` deliberately does not read `FieldBuff`: it identifies
    /// a brace by sniffing `CombatBuff` for `Mitigation` at exactly
    /// `DEFEND_MITIGATION_BONUS`, and a field buff landing on that same
    /// power must not be mistaken for one.
    pub(crate) fn effective_mitigation(&self, entity: Entity) -> i32 {
        let base = self
            .world
            .get::<Stats>(entity)
            .map(|s| s.mitigation)
            .unwrap_or(0);
        let bonus = self
            .world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Mitigation)
            .map(|a| a.power)
            .unwrap_or(0);
        let field_bonus = self.field_buff_power(entity, FieldBuffKind::Mitigation);
        let total = if entity != self.player_entity() {
            base + bonus + field_bonus
        } else {
            base + bonus + field_bonus + self.party_stat_bonus().1 + self.wielded_stat_bonus().1
        };
        total.clamp(0, MAX_MITIGATION_PERCENT)
    }

    /// Standing `(atk, def)` bonus the player gets just for having programs
    /// in their active party — each member contributes 10% of its own
    /// current ATK and DEF (minimum 1 each), summed across the party.
    /// Computed live from each companion's current `Stats` rather than
    /// baked into the player's own `Stats` on add/remove, so it stays
    /// correct automatically as a companion levels up, is fused, or dies —
    /// no separate bookkeeping to keep in sync.
    /// The program currently wielded as the player's weapon, if it still
    /// exists.
    ///
    /// The existence check is the whole point. A program can be sold,
    /// extracted, fused away or killed, and each of those despawns it —
    /// so `resources::WieldedProgram` is allowed to hold a stale entity and
    /// this drops it, using `Stats` as the repo's idiom for "this entity is
    /// gone". Neither `dissolve_tamed_program` nor `fuse_companions` has to
    /// know this feature exists, and a third destruction path added later
    /// inherits the same immunity. Do not "tidy this up" into an explicit
    /// clear at each of those sites — the omission is the design.
    pub(crate) fn wielded_program(&self) -> Option<Entity> {
        self.world
            .get_resource::<WieldedProgram>()
            .and_then(|w| w.0)
            .filter(|&e| self.world.get::<Stats>(e).is_some())
    }

    /// Standing `(atk, def)` bonus the player gets for wielding a program,
    /// or `(0, 0)` when none is wielded. A share of the program's own
    /// current ATK and DEF, floored at 1 each.
    ///
    /// A second, independent knob from `party_stat_bonus` rather than a call
    /// into it: the party buff is a candidate for removal, and this must
    /// survive that. Computed live from the program's current `Stats` for
    /// the reason that function's doc gives — it stays correct as the
    /// program levels, is fused, or dies, with no bookkeeping to sync.
    pub(crate) fn wielded_stat_bonus(&self) -> (i32, i32) {
        self.wielded_program()
            .and_then(|e| self.world.get::<Stats>(e))
            .map(|s| {
                (
                    (s.atk / WIELDED_PROGRAM_STAT_DIVISOR).max(1),
                    (s.mitigation / WIELDED_PROGRAM_STAT_DIVISOR).max(1),
                )
            })
            .unwrap_or((0, 0))
    }

    pub(crate) fn party_stat_bonus(&self) -> (i32, i32) {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.world.get::<Stats>(e))
            .fold((0, 0), |(atk, def), s| {
                (
                    atk + (s.atk / PARTY_PASSIVE_STAT_DIVISOR).max(1),
                    def + (s.mitigation / PARTY_PASSIVE_STAT_DIVISOR).max(1),
                )
            })
    }
}
