//! `AbilityEffect::Tamper`'s other half: writing a `Tampered` entry and
//! ageing it away. `Game::run_tactical_routine` is the one caller of
//! `apply_tamper`; `tactical/turn.rs::hand_on_turn` is the one caller of
//! `age_tamper`. Nothing reads an entry for a decision yet — that is Tasks
//! 3–5.

use crate::abilities::TamperKind;
use crate::tactical::{TacticalBattle, reach};
use crate::tuning::TACTICAL_AI_TEMPERATURE;
use crate::*;

impl Game {
    /// Applies `ability`'s `Tamper` effect, run by `actor` and aimed at
    /// `aim`, to every recipient but the player.
    ///
    /// **The player is never a recipient.** `reach::recipients` reads no
    /// side, so a shaped tamper would otherwise catch the player exactly as
    /// full friendly fire catches a companion — dropped here rather than
    /// filtered by a caller, since every caller shares the one rule.
    ///
    /// **`Hallucinating` is a stub.** Nothing is inserted for it yet — Task
    /// 6 seats the decoys `TacticalBattle` will hold. The take-hold line
    /// still fires for every recipient, the fiction ahead of the mechanic.
    ///
    /// `fresh` is `recipient == actor`: a companion's own radius can catch
    /// its caster, and `TamperEntry`'s first `age` has to know not to spend
    /// the turn that just applied it — Decision 5.
    pub(crate) fn apply_tamper(
        &mut self,
        actor: Entity,
        ability: &AbilityDef,
        kind: TamperKind,
        duration: u32,
        aim: (i32, i32),
    ) {
        let shape = ability.tactical_shape();
        let recipients: Vec<Entity> = {
            let battle = self.world.resource::<TacticalBattle>();
            reach::recipients(battle, actor, aim, shape)
        }
        .into_iter()
        .filter(|&e| self.world.get::<Player>(e).is_none())
        .collect();

        for recipient in recipients {
            if !matches!(kind, TamperKind::Hallucinating { .. }) {
                self.write_tamper_entry(recipient, kind, duration, recipient == actor);
            }
            self.log_tamper_take_hold(recipient, kind);
        }

        if ability.effect.breaks_cloak() {
            self.break_cloak(actor);
        }
    }

    /// `Tampered`'s own `arm_cloak` — inserts the component fresh if
    /// `recipient` carries none yet, otherwise mutates the one already
    /// there. `Tampered::apply` is what actually refreshes the slot.
    fn write_tamper_entry(
        &mut self,
        recipient: Entity,
        kind: TamperKind,
        duration: u32,
        fresh: bool,
    ) {
        match self.world.get_mut::<Tampered>(recipient) {
            Some(mut tampered) => tampered.apply(kind, duration, fresh),
            None => {
                let mut tampered = Tampered::default();
                tampered.apply(kind, duration, fresh);
                self.world.entity_mut(recipient).insert(tampered);
            }
        }
    }

    /// Ages `body`'s `Tampered` entries by one of its own turns, logging a
    /// wear-off line per kind that expired — `tactical/turn.rs::hand_on_turn`'s
    /// one caller, on the tampered body's own hand-on.
    pub(crate) fn age_tamper(&mut self, body: Entity) {
        let Some(mut tampered) = self.world.get_mut::<Tampered>(body) else {
            return;
        };
        let expired = tampered.age();
        if tampered.is_empty() {
            self.world.entity_mut(body).remove::<Tampered>();
        }
        for kind in expired {
            self.log_tamper_wear_off(body, kind);
        }
    }

    /// `entity_label` for a hostile, `creature_label` for a companion —
    /// `tick_combatant_upkeep`'s own split. The player is never a subject
    /// here: `apply_tamper` drops it before either log fires.
    fn tamper_label(&self, body: Entity) -> String {
        if self.world.get::<Hostile>(body).is_some() {
            self.entity_label(body)
        } else {
            self.creature_label(body)
        }
    }

    fn log_tamper_take_hold(&mut self, recipient: Entity, kind: TamperKind) {
        let label = self.tamper_label(recipient);
        let line = match kind {
            TamperKind::Temperature(t) if t <= TACTICAL_AI_TEMPERATURE => {
                format!("{label}'s sampler runs cold.")
            }
            TamperKind::Temperature(_) => format!("{label}'s sampler runs hot."),
            TamperKind::Profiled => format!("{label}'s policy is profiled."),
            TamperKind::Injected => format!("{label} accepts an injected prompt."),
            TamperKind::Hallucinating { .. } => format!("{label} starts seeing decoys."),
        };
        self.log(line);
    }

    fn log_tamper_wear_off(&mut self, body: Entity, kind: TamperKind) {
        let label = self.tamper_label(body);
        let line = match kind {
            TamperKind::Temperature(_) => format!("{label}'s sampler settles."),
            TamperKind::Profiled => format!("{label}'s profile goes stale."),
            TamperKind::Injected => format!("{label} rejects the injection."),
            TamperKind::Hallucinating { .. } => format!("{label} sees clearly again."),
        };
        self.log(line);
    }
}
