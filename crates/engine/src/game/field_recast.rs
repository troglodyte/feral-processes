//! Auto-recast: a holder's flag on a timed field routine, and the rerun of
//! that routine when the buff it armed runs out.

use crate::abilities::AbilityId;
use crate::components::AutoRecast;
use crate::game::field::CastError;
use crate::*;

/// The flagged holder that would rerun an expired buff, found by the gather
/// step in `routine_holders()` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Caster {
    pub holder: Entity,
    pub ability: AbilityId,
    pub target: AbilityTarget,
}

/// One expired timed buff, as the gather step saw it. `caster` is `None` when
/// the buff has no routine on record, or no holder has it installed and
/// flagged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Expiry {
    pub recipient: Entity,
    pub name: String,
    pub caster: Option<Caster>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Recast {
        holder: Entity,
        ability: AbilityId,
        name: String,
        pick: FieldRoutineTarget,
    },
    Fade(String),
}

/// Decides what each expiry becomes. Pure: no `World`, no `Game`, no RNG.
///
/// `expiries` arrive in `tick_field_buffs`' walk order (the player, then
/// `Party`), which is already a stable order, so the plan keeps it.
///
/// Expiries that would cast the same thing collapse into one recast: a
/// `WholeParty` routine expires on every recipient on the same tick and must
/// pay once, and its pick is `None`. A `OneAlly` routine picks its recipient,
/// so two allies expiring together each get their own recast. An expiry with
/// no caster fades as it always did.
///
/// `tick_inner` runs during a battle too (every round calls `tick`), and the
/// recast goes ahead there: it is upkeep, not an action, so the manual
/// cast's "not during a battle" refusal does not apply.
pub(crate) fn plan_recasts(expiries: &[Expiry]) -> Vec<Plan> {
    let mut plans: Vec<Plan> = Vec::new();
    for e in expiries {
        let Some(caster) = &e.caster else {
            plans.push(Plan::Fade(e.name.clone()));
            continue;
        };
        let pick = match caster.target {
            AbilityTarget::OneAlly => FieldRoutineTarget::Ally(e.recipient),
            _ => FieldRoutineTarget::None,
        };
        let plan = Plan::Recast {
            holder: caster.holder,
            ability: caster.ability.clone(),
            name: e.name.clone(),
            pick,
        };
        if !plans.iter().any(|p| match (p, &plan) {
            (
                Plan::Recast {
                    holder: h,
                    ability: a,
                    pick: k,
                    ..
                },
                Plan::Recast {
                    holder: h2,
                    ability: a2,
                    pick: k2,
                    ..
                },
            ) => h == h2 && a == a2 && k == k2,
            _ => false,
        }) {
            plans.push(plan);
        }
    }
    plans
}

impl Game {
    /// The routines `entity` has flagged to rerun, in flag order. Empty when
    /// the holder carries no `AutoRecast`.
    pub(crate) fn auto_recast_of(&self, entity: Entity) -> Vec<crate::abilities::AbilityId> {
        self.world
            .get::<AutoRecast>(entity)
            .map(|a| a.0.clone())
            .unwrap_or_default()
    }

    pub(crate) fn is_auto_recast(&self, entity: Entity, ability: &str) -> bool {
        self.world
            .get::<AutoRecast>(entity)
            .is_some_and(|a| a.0.iter().any(|id| id == ability))
    }

    /// Flags or unflags `ability` on `entity`. The component is created on
    /// the first flag and kept when emptied: absent and empty read the same.
    pub(crate) fn set_auto_recast(&mut self, entity: Entity, ability: &str, on: bool) {
        let mut flags = self.auto_recast_of(entity);
        flags.retain(|id| id != ability);
        if on {
            flags.push(ability.to_string());
        }
        self.world.entity_mut(entity).insert(AutoRecast(flags));
    }

    /// Who would rerun `ability` for a buff that just expired: the first
    /// holder, in `routine_holders()` order, that has it installed and
    /// flagged. The casting holder is not recorded on the buff, so two
    /// holders of one routine with only one flagged resolve to the flagged
    /// one.
    pub(crate) fn recast_caster(&self, holders: &[Entity], ability: &AbilityId) -> Option<Caster> {
        let def = self.world.resource::<AbilityDb>().get(ability)?;
        if !def.auto_recastable() {
            return None;
        }
        holders
            .iter()
            .copied()
            .find(|&h| {
                self.is_auto_recast(h, ability)
                    && self
                        .world
                        .get::<Routines>(h)
                        .is_some_and(|r| r.0.contains(ability))
            })
            .map(|holder| Caster {
                holder,
                ability: ability.clone(),
                target: def.target,
            })
    }

    /// Reruns or fades the buffs `tick_field_buffs` just removed.
    pub(crate) fn resolve_expired_buffs(
        &mut self,
        expired: Vec<(Entity, String, Option<AbilityId>)>,
    ) {
        if expired.is_empty() {
            return;
        }
        let holders: Vec<Entity> = self
            .routine_holders()
            .into_iter()
            .map(|h| h.entity)
            .collect();
        let expiries: Vec<Expiry> = expired
            .into_iter()
            .map(|(recipient, name, ability)| Expiry {
                recipient,
                caster: ability.and_then(|a| self.recast_caster(&holders, &a)),
                name,
            })
            .collect();
        for plan in plan_recasts(&expiries) {
            match plan {
                Plan::Fade(name) => self.log(format!("{name} fades.")),
                Plan::Recast {
                    holder,
                    ability,
                    name,
                    pick,
                } => match self.cast_field_routine(holder, &ability, pick) {
                    Ok(cast) => {
                        self.log(format!("{} reruns {}.", cast.holder_label, cast.name));
                    }
                    Err(CastError::TargetGone(why)) => {
                        let why = why.trim_end_matches('.');
                        self.log(format!("{name} fades — couldn't rerun it: {why}."));
                    }
                    Err(CastError::Refused(why)) => {
                        let why = why.trim_end_matches('.');
                        self.log(format!(
                            "{name} fades — couldn't rerun it: {why}. Auto-recast is off."
                        ));
                        self.set_auto_recast(holder, &ability, false);
                    }
                },
            }
        }
    }
}
