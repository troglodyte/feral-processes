//! Getting out of a battle — by jacking out, or because it is over.
//!
//! Both routes funnel through `end_battle`, which is the only place
//! `BattleState` is dropped. The deferred reap it performs is the reason it
//! exists at all: `BattleState::planned` indexes `Party` positionally, so a
//! member killed mid-fight cannot leave the roster until the fight does.

use crate::tuning::{FLEE_COUNTERATTACK_CHANCE, JACK_OUT_LUCK_MAX, JACK_OUT_LUCK_MIN};
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
        // Collected before `end_battle` drops `BattleState` — this is the
        // pack that was actually in the fight, not every pursuer in the
        // zone. Only these lose `Pursuing`; a guardian still walking in
        // from elsewhere keeps chasing.
        let battle_members: Vec<Entity> = self
            .world
            .resource::<BattleState>()
            .groups
            .iter()
            .flat_map(|g| g.members.iter().copied())
            .collect();
        let front = self.front_of_group(0);
        self.end_battle(player, front);
        // A successful jack-out shakes the pack that caught you: without
        // this, `nest_aggro_tick` (inside the `tick` below) would find the
        // same guardians still adjacent and still `Pursuing`, and
        // re-engage before the player's next input ever arrived — under
        // permadeath, every attempt to leave would cost the XP setback
        // above for nothing. `NestGuardian` is untouched, so a cleared
        // guardian stays tethered and resumes ordinary wandering, exactly
        // like a `despawn_nest` survivor; the nest re-provokes it the next
        // time `attack_nest` lands a hit. A failed attempt (the branch
        // above) shakes nobody.
        for member in battle_members {
            if let Ok(mut entity) = self.world.get_entity_mut(member) {
                entity.remove::<Pursuing>();
            }
        }
        self.tick();
        true
    }

    /// Clears any residual status effects, combat buffs, and ability
    /// cooldowns from the player, every party member, and every hostile
    /// still in the fight. Status conditions are scoped to a single
    /// intrusion, so nothing should carry forward once one ends, however it
    /// ends. `wild` is `None` when the pack is already gone, and may name an
    /// entity that has already left its group (a decompile) or already
    /// despawned (a kill) and so isn't reachable through
    /// `all_living_enemies` — in which case clearing it again is a no-op,
    /// but neither case may skip clearing your own side.
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
        // Every hostile still in the fight, not only the one passed in.
        // Survivors of a jack-out stay on the map, and a mirrored buff left
        // armed on one never ticks down — `effective_atk`/`effective_def`
        // read `CombatBuff` unconditionally, so it would be a free stat
        // forever. `wild` is still taken because it may name a program that
        // has already left its group (a successful decompile).
        let mut hostiles: Vec<Entity> = self.all_living_enemies();
        hostiles.extend(wild);
        for hostile in hostiles {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(hostile) {
                s.active = None;
            }
            if let Some(mut b) = self.world.get_mut::<CombatBuff>(hostile) {
                b.active = None;
            }
            if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(hostile) {
                c.0.clear();
            }
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
    /// both sides, companions killed during the fight finally reaped, and
    /// `BattleState` dropped.
    ///
    /// Reaping the dead happens here rather than the moment they fall
    /// because `BattleState::planned` indexes `Party` positionally (see
    /// `actor_entity`) — removing a member mid-battle shifts every member
    /// behind it into the wrong slot. The death itself is announced when it
    /// happens, in `apply_damage`; only the despawn waits.
    ///
    /// `wild` is passed in rather than looked up because two of the callers
    /// have already popped the group: the entity whose status must be
    /// cleared is the one that just died or was decompiled, not whatever
    /// stepped up behind it. A freshly tamed program joining the party still
    /// Bleeding is the bug this guards.
    pub(crate) fn end_battle(&mut self, player: Entity, wild: Option<Entity>) {
        // First, deliberately: `dissolve_tamed_program` below drops the dead
        // out of `Party` and despawns them, and a companion that died
        // winning the fight is the one thing the results page most needs to
        // report. A copy, not a live read — the entities are gone by the
        // time anything draws it.
        let closing = self
            .battle_rows()
            .map(|(_, party)| party)
            .unwrap_or_default();
        self.world.resource_mut::<BattleTimeline>().closing = closing;
        self.clear_battle_status_effects(player, wild);
        let dead: Vec<Entity> = self
            .world
            .resource::<Party>()
            .0
            .iter()
            .copied()
            .filter(|&e| !self.creature_alive(e))
            .collect();
        // Before `retain_outcomes_since_battle` below, deliberately: the
        // detachment lines `dissolve_tamed_program` writes are `Info` kind,
        // so running the reap first is what prunes them and leaves the
        // `Outcome` death line to reach the map alone.
        for program in dead {
            self.dissolve_tamed_program(program);
        }
        // A Stack pack that outlived the fight — the party jacked out —
        // has nowhere to go: it stands at surface coordinates around the
        // link mouth, and would be waiting there when they climb back out.
        //
        // `Without<Tamed>` is load-bearing, not defensive: decompiling one of
        // these mid-fight makes it the player's, and sweeping it up with the
        // rest would delete a program they just earned.
        let strays: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, (With<StackSpawn>, Without<Tamed>)>();
            query.iter(&self.world).collect()
        };
        for stray in strays {
            self.world.despawn(stray);
        }
        // The blow-by-blow has done its job by now — the battle pane showed
        // it live. What follows the player onto the map is the results.
        self.world
            .resource_mut::<MessageLog>()
            .retain_outcomes_since_battle();
        // The prune deletes lines from inside the range the frames are
        // counted against, so every one of them now points at the wrong
        // line. There is no roster left to draw them on either — the
        // screen is gone with `BattleState`.
        self.world.resource_mut::<BattleTimeline>().frames.clear();
        self.world.remove_resource::<BattleState>();
    }
}
