//! What a combat skill level is worth at each door, and the hooks that
//! practise one (ADR 0040). `skills/` stays blind to combat: the pure effect
//! functions here are the whole of "what a level buys", and every one is the
//! identity at `SKILL_NEUTRAL_LEVEL`. A body without `Skills` reads as that
//! level everywhere, so it is byte-identical to today.

use crate::resources::PendingSkillLevels;
use crate::skills::{self, Skill, Skills};
use crate::tuning::{
    AIM_PRACTICE_PER_ROLL, COMBAT_SKILL_SPREAD, DEFENCE_PRACTICE_PER_ROLL, FORCE_PRACTICE_PER_HIT,
    ROUTINES_PRACTICE_PER_CHARGE, SKILL_NEUTRAL_LEVEL,
};
use crate::*;

/// Multiplier on a body's accuracy.
pub(crate) fn aim_scale(level: u32) -> f64 {
    f64::from(skills::scale(level, COMBAT_SKILL_SPREAD))
}

/// Multiplier on a landed hit's damage.
pub(crate) fn force_scale(level: u32) -> f64 {
    f64::from(skills::scale(level, COMBAT_SKILL_SPREAD))
}

/// Mitigation points, added like a gear point.
pub(crate) fn defence_points(level: u32) -> i32 {
    level as i32 - SKILL_NEUTRAL_LEVEL as i32
}

/// Multiplier on a routine's Power price: dearer when rusty, cheaper when
/// practised.
pub(crate) fn routine_price_scale(level: u32) -> f32 {
    2.0 - skills::scale(level, COMBAT_SKILL_SPREAD)
}

impl Game {
    /// `entity`'s level in a combat skill, `None` for a body that carries no
    /// `Skills`.
    pub(crate) fn combat_skill_level(&self, entity: Entity, skill: Skill) -> Option<u32> {
        self.world
            .get::<Skills>(entity)
            .map(|skills| skills.level(skill))
    }

    /// What `def` costs `entity` to run: the one price, for the refusal, the
    /// charge and the routine page alike.
    pub(crate) fn routine_cost(&self, entity: Entity, def: &crate::abilities::AbilityDef) -> f32 {
        crate::abilities::routine_power_cost(def, self.combat_skill_level(entity, Skill::Routines))
    }

    /// Adds `amount` practice to a combat skill of `entity`; a no-op for a
    /// body without `Skills`. A level gained queues the shared announce.
    pub(crate) fn practise_combat(&mut self, entity: Entity, skill: Skill, amount: u32) {
        let gained = {
            let Some(mut skills) = self.world.get_mut::<Skills>(entity) else {
                return;
            };
            let outcome = skills::practise(skills.track_mut(skill), amount);
            (outcome.levels_gained > 0).then_some(outcome)
        };
        if let Some(outcome) = gained {
            // One entry per level, so no number is skipped.
            let first = outcome.new_level - outcome.levels_gained;
            let mut pending = self.world.resource_mut::<PendingSkillLevels>();
            for level in first + 1..=outcome.new_level {
                pending.0.push((entity, skill, level));
            }
        }
    }

    /// The practice one attack roll earns: Aim for the attacker, Defence for
    /// the defender, and Force for the attacker when it landed.
    pub(crate) fn practise_attack_roll(
        &mut self,
        attacker: Entity,
        defender: Entity,
        landed: bool,
    ) {
        self.practise_combat(attacker, Skill::Aim, AIM_PRACTICE_PER_ROLL);
        if landed {
            self.practise_combat(attacker, Skill::Force, FORCE_PRACTICE_PER_HIT);
        }
        self.practise_combat(defender, Skill::Defence, DEFENCE_PRACTICE_PER_ROLL);
    }

    /// The practice one paid routine charge earns.
    pub(crate) fn practise_routine_charge(&mut self, entity: Entity) {
        self.practise_combat(entity, Skill::Routines, ROUTINES_PRACTICE_PER_CHARGE);
    }
}
