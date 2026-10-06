//! Calling a Drop Trooper into a battle map — drop pods phase B2
//! (`docs/superpowers/archive/specs/2026-09-27-drop-pods-design.md` §4–6).
//!
//! `reinforcement_pairing` is the one derivation of *who drops through
//! which pod*: `reinforcement_refusal` asks it ahead of the charge and
//! `call_reinforcement` acts on it after, so the two cannot disagree.

use crate::components::{DropPod, Reinforcement};
use crate::game::base::floor::drop_load;
use crate::game::base::offshift::Amenities;
use crate::game::combat::RoutineRefusal;
use crate::resources::Party;
use crate::tactical::TacticalBattle;
use crate::*;

impl Game {
    /// Every charged pod and the base-space tile it stands on, in `(x, y)`
    /// order so a tie between two equally near pods resolves the same way
    /// every run — bevy's query order is not stable.
    fn charged_pods(&self) -> Vec<(Entity, (i32, i32))> {
        let mut pods: Vec<(Entity, (i32, i32))> = self
            .world
            .iter_entities()
            .filter(|e| e.get::<DropPod>().is_some_and(|p| p.charged))
            .filter_map(|e| e.get::<Position>().map(|p| (e.id(), (p.x, p.y))))
            .collect();
        pods.sort_by_key(|&(e, (x, y))| (x, y, e));
        pods
    }

    /// `Game::amenities` from a `&Game` — the refusal is asked from
    /// `ability_unavailable`, which cannot take `&mut`. `Amenities::build`
    /// takes an iterator for exactly this.
    pub(crate) fn amenities_here(&self) -> Amenities {
        let sites: Vec<(StructureId, Position)> = self
            .world
            .iter_entities()
            .filter_map(|e| Some((e.get::<Structure>()?.kind.clone(), *e.get::<Position>()?)))
            .collect();
        Amenities::build(
            sites.iter().map(|(kind, p)| (kind, p)),
            self.world.resource::<StructureDb>(),
        )
    }

    /// The Drop Troopers a pod may call right now, in table order:
    /// `drop_troopers` (Staff only, by omission) that are on shift.
    ///
    /// **One exclusion beyond `is_on_shift`**: a body carrying a downed
    /// program. That predicate keeps a carrier on shift because freeing one
    /// mid-trip destroys what it holds, and a call frees it — a stack of
    /// fragments is the scheduler's own accepted loss (`pin_subject`'s
    /// rule), a carried program is a kill the player cannot get back.
    fn callable_troopers(&self) -> Vec<Entity> {
        let amenities = self.amenities_here();
        self.drop_troopers()
            .into_iter()
            .filter(|&e| self.is_on_shift(e, &amenities))
            .filter(|&e| {
                self.world
                    .get::<crate::components::CarryingProgram>(e)
                    .is_none()
            })
            .collect()
    }

    /// Who drops and through which pod: each callable trooper paired with
    /// its nearest charged pod by base-space Chebyshev distance, the
    /// closest pair winning and a tie going to the Base staff table's
    /// order. No `GameRng` draw.
    ///
    /// `None` answers two different refusals, which the caller tells apart
    /// by asking `charged_pods` first.
    pub(crate) fn reinforcement_pairing(&self) -> Option<(Entity, Entity)> {
        let pods = self.charged_pods();
        self.callable_troopers()
            .into_iter()
            .enumerate()
            .filter_map(|(rank, trooper)| {
                let at = self.world.get::<Position>(trooper)?;
                let (distance, pod) = pods
                    .iter()
                    .map(|&(pod, (x, y))| ((x - at.x).abs().max((y - at.y).abs()), pod))
                    .min_by_key(|&(distance, _)| distance)?;
                Some(((distance, rank), trooper, pod))
            })
            .min_by_key(|&(key, ..)| key)
            .map(|(_, trooper, pod)| (trooper, pod))
    }

    /// Why `invoker` cannot call a reinforcement now, or `None` — the four
    /// refusals `ability_unavailable` asks ahead of Power, so the price is
    /// never charged for a call that seats nobody.
    ///
    /// The siege first: whatever else is true, a siege's staff are already
    /// on the board.
    pub(crate) fn reinforcement_refusal(&self, invoker: Entity) -> Option<RoutineRefusal> {
        if self
            .world
            .get_resource::<TacticalBattle>()
            .is_some_and(|b| b.siege_pack > 0)
        {
            return Some(RoutineRefusal::InSiege);
        }
        if self.charged_pods().is_empty() {
            return Some(RoutineRefusal::NoPodReady);
        }
        if self.reinforcement_pairing().is_none() {
            return Some(RoutineRefusal::NoTrooperAvailable);
        }
        if !self.board_has_room(invoker) {
            return Some(RoutineRefusal::NoLandingCell);
        }
        None
    }

    /// Drops the paired trooper beside `invoker`: seats it behind the
    /// cursor, spends its pod (which files the recharge), unposts it, and
    /// pushes it to `Party` **without** the `MAX_PARTY_SIZE` check —
    /// `seat_summon_in_group`'s precedent, and the point of calling one.
    ///
    /// **Unposting is by hand**, `add_companion`'s and `pin_subject`'s
    /// reason: the role derivation stops the scheduler posting it again,
    /// but a non-staff body still holding a `Task` is an outsider whose
    /// post the scheduler leaves covered — and the scheduler does not run
    /// at all while a fight is open.
    ///
    /// Reports whether a trooper landed.
    pub(crate) fn call_reinforcement(&mut self, invoker: Entity) -> bool {
        let Some((trooper, pod)) = self.reinforcement_pairing() else {
            return false;
        };
        if !self.seat_summon_on_board(invoker, trooper) {
            return false;
        }
        self.spend_pod(pod);
        // The load stays behind on the base tile: the trooper's `Position` is
        // still the base cell it was called from.
        drop_load(&mut self.world, trooper);
        self.world
            .entity_mut(trooper)
            .remove::<Task>()
            .insert(Reinforcement { reorienting: true });
        self.world.resource_mut::<Party>().0.push(trooper);
        self.cue_tactical_fx(trooper, crate::resources::TacticalFxKind::Landing);
        let name = self.creature_label(trooper);
        self.log(format!("{name} drops in from the base, reorienting."));
        true
    }

    /// Passes a reinforcement's first turn, called from `hand_on_turn`
    /// right after the order moves on — the way `skip_disengaged_turns`
    /// passes an idle siege body, and for the reason `Reinforcement`'s own
    /// doc gives: a `Stun` would not cost it the turn on a battle map.
    ///
    /// `insert_after_cursor` put it directly behind the invoker, so the
    /// hand-on that ends the invoking turn is the one that lands here.
    pub(crate) fn pass_reorienting_turn(&mut self) {
        let Some(actor) = self
            .world
            .get_resource::<TacticalBattle>()
            .and_then(|b| b.actor())
        else {
            return;
        };
        if !self
            .world
            .get::<Reinforcement>(actor)
            .is_some_and(|r| r.reorienting)
        {
            return;
        }
        self.world
            .entity_mut(actor)
            .insert(Reinforcement { reorienting: false });
        let name = self.creature_label(actor);
        self.log(format!("{name} gets its bearings."));
        self.world.resource_mut::<TacticalBattle>().end_turn();
    }

    /// Takes every `Reinforcement` holder out of `Party` and strips the
    /// marker — `finish_fight`'s last word on a call, so the party is back
    /// to its size and the trooper back to base staff.
    pub(crate) fn send_reinforcements_home(&mut self) {
        let troopers: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, bevy_ecs::prelude::With<Reinforcement>>();
            query.iter(&self.world).collect()
        };
        for trooper in troopers {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != trooper);
            self.world.entity_mut(trooper).remove::<Reinforcement>();
        }
    }
}
