//! Routines that fire on an event instead of being chosen on a turn.
//!
//! Every other routine in the game reaches `Game::use_ability` because
//! somebody picked it: the player off `battle_special_options`, a wild
//! carrier off `wild_routine_ready`, the field caster off `field_routines`.
//! A passive is the one that reaches it because something *happened* — a
//! party member dropped, a status landed — and that is the whole of what
//! this module is.
//!
//! **The trigger set is closed, and deliberately small.** Each
//! `PassiveTrigger` variant is a specific point in `battle_resolve_round`
//! that calls `fire_passives`, so a variant nobody fires would be an
//! authored routine that silently never runs. Adding one means adding the
//! call site in the same change — which is why `AbilityDb::load_dir` refuses
//! a `triggers` on a field-only effect rather than warning about it.
//!
//! **Passives are excluded from all three menus** (`battle_special_options`,
//! `field_routines`, `wild_routine_ready`) through `AbilityDef::is_passive`.
//! They occupy a slot like anything else and cost the holder their pick of
//! what else could be in it; what they never cost is a turn.

use crate::abilities::{AbilityDef, PassiveTrigger};
use crate::battle;
use crate::components::{Routines, StatusEffects};
use crate::*;

impl Game {
    /// Fires every passive installed on `holders` whose trigger is
    /// `trigger`, in party order.
    ///
    /// `holders` is who the event happened *to*, which is not always who
    /// reacts: `AllyDropped` passes the whole living party, because a group
    /// losing a member is a fact about the group, while `Afflicted` passes
    /// only the combatant the status landed on. Deciding that at the call
    /// site rather than here keeps this function ignorant of what any
    /// particular trigger means.
    ///
    /// Cooldowns are honoured and armed exactly as `resolve_one_action`
    /// does, and for the same reason: a passive with `cooldown: 0` fires
    /// every single round it is triggered, which for `AllyDropped` is a
    /// wipe-in-progress and for `Afflicted` is any enemy with a status
    /// rider. Both shipped passives carry one.
    ///
    /// Armed *before* the effect resolves, which is `resolve_one_action`'s
    /// ordering and load-bearing for the same reason: a killing blow ends
    /// the battle inside `reap_dead_members`, and `end_battle` wipes every
    /// battle-scoped component — so a cooldown written afterwards would land
    /// on an entity already cleaned up.
    pub(crate) fn fire_passives(&mut self, trigger: PassiveTrigger, holders: &[Entity]) {
        for &holder in holders {
            if !self.creature_alive(holder) {
                continue;
            }
            for ability in self.ready_passives(holder, trigger) {
                // Re-checked inside the loop rather than once above it: an
                // earlier passive on the same holder can end the battle, and
                // a second one resolving into a torn-down `BattleState` is
                // the same crash `battle_resolve_round`'s own loop guards
                // against between actors.
                if self.world.get_resource::<BattleState>().is_none() {
                    return;
                }
                if !self.creature_alive(holder) {
                    break;
                }
                self.arm_cooldown(holder, &ability);
                let recipients =
                    self.ability_recipients(holder, ability.target, &battle::SpecialTarget::WholeParty);
                if recipients.is_empty() {
                    continue;
                }
                let name = self.routine_holder_label(holder);
                self.log_kind(
                    MessageKind::Round,
                    format!("{name}'s {} cuts in.", ability.name),
                );
                self.use_ability(&ability, holder, &name, &recipients);
            }
        }
    }

    /// The passives on `holder` matching `trigger` that are not on
    /// cooldown, in slot order.
    ///
    /// Collected up front rather than iterated lazily because firing one
    /// mutates the world underneath the `Routines` borrow.
    fn ready_passives(&self, holder: Entity, trigger: PassiveTrigger) -> Vec<AbilityDef> {
        let cooling = self
            .world
            .get::<AbilityCooldowns>(holder)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(holder)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|id| !cooling.contains_key(*id))
            .filter_map(|id| db.get(id))
            .filter(|def| def.triggers == Some(trigger))
            .cloned()
            .collect()
    }

    /// Which living party members took a status this round — the holders an
    /// `Afflicted` passive fires for.
    ///
    /// Keyed on `ActiveStatus::landed_this_round`, the flag
    /// `tick_round_status_effects` already maintains so a status does not
    /// burn a round of its own duration in the round it arrived. Reading it
    /// here means "afflicted" is *newly* afflicted: a routine that fired
    /// again every round of a four-round Bleed would be four casts for one
    /// event.
    ///
    /// Called before `tick_round_status_effects` clears the flag, which is
    /// the one ordering constraint this file has on its caller.
    pub(crate) fn newly_afflicted_party(&self) -> Vec<Entity> {
        self.living_party()
            .into_iter()
            .filter(|&e| {
                self.world
                    .get::<StatusEffects>(e)
                    .and_then(|s| s.active.as_ref())
                    .is_some_and(|status| status.landed_this_round)
            })
            .collect()
    }
}
