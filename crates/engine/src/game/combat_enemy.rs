//! What the other side does with its round: who it swings at, and whether
//! it spends the round on a routine instead.
//!
//! `all_wild_retaliate` (`game/combat_round.rs`) drives this once per
//! engaged group; everything about *choosing* the strike lives here.

use crate::tuning::{ENEMY_ROUTINE_MIN_COOLDOWN, WILD_ABILITY_CHANCE};
use crate::*;

impl Game {
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

    /// The routine `wild` will spend this round on, if it is carrying one
    /// that is not still cooling.
    ///
    /// First installed wins. A carrier holds exactly one
    /// (`Game::roll_wild_routine`), so ordering is not a real decision, and
    /// inventing a priority scheme for a one-element list would be building
    /// for a case that does not exist.
    ///
    /// `Decompile` is excluded: it is resolved by group index against the
    /// *wild* side and would do nothing coherent aimed the other way. Only a
    /// mod can put it on a hostile — `decompile.ron` has no `wild_weight` —
    /// but a mod that does gets a normal move rather than a wasted round.
    ///
    /// Every **field-only** effect is excluded the same way (see
    /// `AbilityEffect::field_only`): none has a battle mechanic to run, so a
    /// carrier with nothing else installed falls back to a normal move
    /// instead of the `unreachable!` in `use_ability`.
    pub(crate) fn wild_routine_ready(&self, wild: Entity) -> Option<AbilityDef> {
        let cooling = self
            .world
            .get::<AbilityCooldowns>(wild)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(wild)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|id| !cooling.contains_key(*id))
            .filter_map(|id| db.get(id))
            .find(|def| {
                !matches!(def.effect, AbilityEffect::Decompile)
                    && !def.effect.field_only()
                    && !def.is_passive()
            })
            .cloned()
    }

    /// The wild creature strikes back at whoever's exposed: normally the
    /// player or a party member, weighted by slot — see `roll_enemy_target`.
    pub(crate) fn wild_retaliate(&mut self, wild: Entity, group: usize, player: Entity) {
        // A carrier spends its round on the routine rather than a move. No
        // engagement check: `ENGAGED_GROUPS` gates *moves* because a
        // back-rank program has to physically reach, and a routine is
        // executed rather than swung — gating it would silently disable
        // every carrier behind the front groups.
        if let Some(routine) = self.wild_routine_ready(wild) {
            // Armed before the effect resolves, the same reason
            // `resolve_one_action` arms early: a killing blow ends the
            // battle inside `reap_dead_members` and `end_battle` wipes every
            // battle-scoped component, so a cooldown written afterwards
            // would land on an entity already cleaned up.
            //
            // Floored at `ENEMY_ROUTINE_MIN_COOLDOWN` — see
            // `abilities::armed_cooldown`, the one function both sides call.
            let armed = abilities::armed_cooldown(routine.cooldown, ENEMY_ROUTINE_MIN_COOLDOWN);
            let mut cooldowns = self
                .world
                .get::<AbilityCooldowns>(wild)
                .map(|c| c.0.clone())
                .unwrap_or_default();
            cooldowns.insert(routine.id.clone(), armed);
            self.world
                .entity_mut(wild)
                .insert(AbilityCooldowns(cooldowns));

            let name = self.creature_label(wild);
            self.log_kind(
                MessageKind::EnemySpecial,
                format!("{name} runs {}.", routine.name),
            );
            // A single-target routine is aggro-weighted exactly like a wild
            // move — see `roll_enemy_target` — so bracing and slot order
            // still matter against it. Rolled only for the shape that needs
            // a single party-side pick; every other shape in
            // `ability_recipients`' hostile branch ignores `chosen`.
            let chosen = if matches!(routine.target, AbilityTarget::OneEnemyGroupFront) {
                let target = self.roll_enemy_target(player);
                let slot = self.party_slot_of(target).unwrap_or(0);
                battle::SpecialTarget::Ally { slot }
            } else {
                battle::SpecialTarget::EnemyGroup { group }
            };
            let recipients = self.ability_recipients(wild, routine.target, &chosen);
            self.use_ability(&routine, wild, &name, &recipients);
            self.reap_dead_members(player);
            return;
        }

        // Which move and which target are one decision, made in
        // `Game::choose_wild_action` (`game/combat_policy.rs`) — the trained
        // policy scores the pairs jointly, and with no policy installed that
        // is the uniform move roll and the aggro-weighted target roll this
        // used to make inline. `None` is "nothing it has reaches from where
        // it stands", which only a back group with no ranged move can be.
        let Some((mut mv, target)) = self.choose_wild_action(wild, group, player) else {
            let name = self.creature_label(wild);
            self.log(format!("{name} circles beyond reach, unable to strike."));
            return;
        };
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
        // move theoretically has one". Taken before the gate, a Crawler would
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
}
