//! A breeding visit: both parents walk to the Breeding Bay, pause together,
//! and only then does the child start incubating.
//!
//! The slot is reserved at confirm (`Game::breed`) as an `Incubation` whose
//! `visit` is `Some`; the parents carry `components::BreedingVisit` and read
//! as `ProgramRole::Visiting`. `step_breeding_visits` is gather / decide /
//! apply, and `cancel_breeding_visit` is the one door every ending other than
//! completion goes through.

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::breeding::VisitPause;
use crate::components::{
    BreedReadyAt, BreedingVisit, Downed, Incubator, Inventory, MemorySubject, Position, ProgramId,
    Structure, Tamed,
};
use crate::game::base::hauling::{NoPost, Step, Touch, at_station, step_to_post};
use crate::game::breeding::BREEDING_SEED;
use crate::game::party::ProgramRole;
use crate::items::ItemId;
use crate::resources::GameClock;
use crate::tuning::BREEDING_VISIT_STUCK_TICKS;
use bevy_ecs::prelude::Entity;

/// Why a visit was called off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CancelReason {
    /// Fewer than two marked parents remain: released, fused, despawned.
    ParentGone,
    /// A parent's role is no longer `Visiting`: it joined the party. Sorties,
    /// outposts, siphons and study all refuse a non-`Staff` body, so none of
    /// them can start on a parent.
    LeftBase,
    /// A parent was benched.
    Downed,
    /// The bay was demolished or destroyed.
    BayLost,
    /// A parent found no route to the bay for too long.
    Unreachable,
}

impl CancelReason {
    fn phrase(self) -> &'static str {
        match self {
            CancelReason::ParentGone => "a parent is gone",
            CancelReason::LeftBase => "a parent was called away",
            CancelReason::Downed => "a parent is hurt",
            CancelReason::BayLost => "the bay is gone",
            CancelReason::Unreachable => "a parent couldn't reach the bay",
        }
    }
}

/// What one beat of the walk did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Progress {
    /// Arrived, or took a step toward arriving.
    Moved,
    /// Neither: waiting on a side, or the final step was refused.
    Stalled,
}

/// What the decide step sees of one parent.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParentState {
    pub role: Option<ProgramRole>,
    pub downed: bool,
    pub arrived: bool,
    /// No arrival and no step for `BREEDING_VISIT_STUCK_TICKS` beats running.
    pub unreachable: bool,
}

/// One pending slot, gathered.
#[derive(Clone, Debug)]
pub(crate) struct VisitInput {
    pub bay: Entity,
    pub slot: usize,
    pub parents: Vec<ParentState>,
    pub pause_left: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisitOutcome {
    Wait,
    Tick,
    Complete,
    Cancel(CancelReason),
}

/// The decision for each visit, in input order. Pure: no `World`, no RNG.
pub(crate) fn decide_visits(inputs: &[VisitInput]) -> Vec<VisitOutcome> {
    inputs
        .iter()
        .map(|visit| {
            if visit.parents.len() < 2 {
                return VisitOutcome::Cancel(CancelReason::ParentGone);
            }
            if visit
                .parents
                .iter()
                .any(|p| p.role != Some(ProgramRole::Visiting))
            {
                return VisitOutcome::Cancel(CancelReason::LeftBase);
            }
            if visit.parents.iter().any(|p| p.downed) {
                return VisitOutcome::Cancel(CancelReason::Downed);
            }
            if visit.parents.iter().any(|p| p.unreachable) {
                return VisitOutcome::Cancel(CancelReason::Unreachable);
            }
            if !visit.parents.iter().all(|p| p.arrived) {
                VisitOutcome::Wait
            } else if visit.pause_left == 0 {
                VisitOutcome::Complete
            } else {
                VisitOutcome::Tick
            }
        })
        .collect()
}

impl Game {
    /// Every program on a breeding visit, for `drift_idle_staff`'s walking
    /// pool: not `Staff`, so `base_staff` omits them.
    pub(crate) fn visiting_bodies(&mut self) -> Vec<Entity> {
        let player = self.player_entity();
        let mut query = self.world.query::<(Entity, &Tamed, &BreedingVisit)>();
        let mut bodies: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, tamed, _)| tamed.owner == player)
            .map(|(e, ..)| e)
            .collect();
        bodies.sort();
        bodies
    }

    /// One step of `worker` toward a side of its bay. `Ok` on arrival and
    /// when it must wait for a free side; `Err` only when there is no bay or
    /// no route, which the drift drops, holding the body where it stands.
    /// Every beat that neither arrives nor takes a step — `NoRoute`,
    /// `BoxedIn` or a refused final step — is counted on the marker, and the
    /// gather reads a long run as the parent being unreachable.
    pub(crate) fn step_to_bay(&mut self, worker: Entity) -> Result<(), NoPost> {
        let result = self.walk_to_bay(worker);
        if let Some(mut visit) = self.world.get_mut::<BreedingVisit>(worker) {
            visit.stuck = match result {
                Ok(Progress::Moved) => 0,
                _ => visit.stuck.saturating_add(1),
            };
        }
        result.map(|_| ())
    }

    fn walk_to_bay(&mut self, worker: Entity) -> Result<Progress, NoPost> {
        let visit = *self
            .world
            .get::<BreedingVisit>(worker)
            .ok_or(NoPost::NoRoute)?;
        let bay = self
            .world
            .get::<Position>(visit.bay)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        if at_station(here, bay, 1, Touch::Sides) {
            return Ok(Progress::Moved);
        }
        // The player's tile is taken for the choice of side: `blocked_tiles`
        // leaves it out, and a side picked under the player refuses the
        // final step. Claimed before `rigid`, which folds bodies into walls
        // (`step_to_repair`'s reason for `rigid` at all).
        let mut blocked = self.blocked_tiles();
        if let Some(player_at) = self.field_pos() {
            blocked.claim(player_at);
        }
        let blocked = blocked.rigid();
        let grid = self.world.resource::<BaseGrid>();
        let step = step_to_post(grid, here, bay, 1, &blocked, grid.radius(), Touch::Sides)?;
        if let Some(Step { to, .. }) = step
            && self.field_pos() != Some((to.x, to.y))
            && let Some(mut pos) = self.world.get_mut::<Position>(worker)
        {
            *pos = to;
            return Ok(Progress::Moved);
        }
        Ok(Progress::Stalled)
    }

    /// The marked parents of one reserved slot, in entity order.
    fn visit_parents(&mut self, bay: Entity, slot: usize) -> Vec<Entity> {
        let mut query = self.world.query::<(Entity, &BreedingVisit)>();
        let mut parents: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, v)| v.bay == bay && v.slot == slot)
            .map(|(e, _)| e)
            .collect();
        parents.sort();
        parents
    }

    /// Slots of `bay` still waiting on their parents.
    fn pending_visit_slots(&self, bay: Entity) -> Vec<usize> {
        self.world
            .get::<Incubator>(bay)
            .map_or_else(Vec::new, |inc| {
                inc.slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.as_ref().is_some_and(|c| c.visit.is_some()))
                    .map(|(i, _)| i)
                    .collect()
            })
    }

    fn gather_visits(&mut self) -> Vec<VisitInput> {
        let mut bays: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &Incubator, &Structure)>();
            query.iter(&self.world).map(|(e, ..)| e).collect()
        };
        bays.sort();
        let mut inputs = Vec::new();
        for bay in bays {
            let bay_at = self.world.get::<Position>(bay).copied();
            for slot in self.pending_visit_slots(bay) {
                let pause_left = self
                    .world
                    .get::<Incubator>(bay)
                    .and_then(|inc| inc.slots[slot].as_ref())
                    .and_then(|c| c.visit)
                    .map_or(0, |v| v.pause_left);
                let parents = self
                    .visit_parents(bay, slot)
                    .into_iter()
                    .map(|p| ParentState {
                        role: self.program_role(p),
                        downed: self.world.get::<Downed>(p).is_some(),
                        unreachable: self
                            .world
                            .get::<BreedingVisit>(p)
                            .is_some_and(|v| v.stuck >= BREEDING_VISIT_STUCK_TICKS),
                        arrived: matches!(
                            (self.world.get::<Position>(p).copied(), bay_at),
                            (Some(here), Some(at)) if at_station(here, at, 1, Touch::Sides)
                        ),
                    })
                    .collect();
                inputs.push(VisitInput {
                    bay,
                    slot,
                    parents,
                    pause_left,
                });
            }
        }
        inputs
    }

    /// Advances every reserved slot one tick. Paused under the same gate as
    /// hatching, so a battle holds a visit exactly where it stands.
    pub(crate) fn step_breeding_visits(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        self.drop_stale_visit_marks();
        let inputs = self.gather_visits();
        let outcomes = decide_visits(&inputs);
        for (input, outcome) in inputs.iter().zip(outcomes) {
            match outcome {
                VisitOutcome::Wait => {}
                VisitOutcome::Tick => self.set_pause(input, input.pause_left - 1),
                VisitOutcome::Complete => self.complete_breeding_visit(input.bay, input.slot),
                VisitOutcome::Cancel(reason) => {
                    self.cancel_breeding_visit(input.bay, input.slot, reason);
                }
            }
        }
    }

    fn set_pause(&mut self, input: &VisitInput, pause_left: u64) {
        if let Some(mut inc) = self.world.get_mut::<Incubator>(input.bay)
            && let Some(child) = inc.slots[input.slot].as_mut()
        {
            child.visit = Some(VisitPause { pause_left });
        }
    }

    /// A marker whose slot is no longer pending — the bay is gone, or the
    /// slot was cleared some other way. It names nothing to refund, so it is
    /// simply removed.
    fn drop_stale_visit_marks(&mut self) {
        let marks: Vec<(Entity, BreedingVisit)> = {
            let mut query = self.world.query::<(Entity, &BreedingVisit)>();
            query.iter(&self.world).map(|(e, v)| (e, *v)).collect()
        };
        for (parent, mark) in marks {
            if !self.pending_visit_slots(mark.bay).contains(&mark.slot) {
                self.world.entity_mut(parent).remove::<BreedingVisit>();
            }
        }
    }

    fn complete_breeding_visit(&mut self, bay: Entity, slot: usize) {
        let parents = self.visit_parents(bay, slot);
        let now = self.world.resource::<GameClock>().tick;
        if let Some(mut incubator) = self.world.get_mut::<Incubator>(bay)
            && let Some(child) = incubator.slots[slot].as_mut()
        {
            child.visit = None;
            child.due = now + crate::tuning::INCUBATION_TICKS;
        }
        for &parent in &parents {
            self.world
                .entity_mut(parent)
                .remove::<BreedingVisit>()
                .insert(BreedReadyAt(now + crate::tuning::BREEDING_COOLDOWN_TICKS));
        }
        if let [a, b] = parents[..] {
            for (who, other) in [(a, b), (b, a)] {
                if let Some(&id) = self.world.get::<ProgramId>(other) {
                    self.remember(who, "bred_with", MemorySubject::Program(id));
                }
            }
            let line = format!(
                "{} and {} head back to work. A child starts to incubate.",
                self.creature_label(a),
                self.creature_label(b)
            );
            self.log_base(line);
        }
    }

    /// Calls off the visit reserved in `slot`: the one door for every ending
    /// but completion. Clears the slot, frees the parents, returns the seed
    /// and says so — no memory forms and no cooldown starts. A slot that is
    /// not pending is left alone, so a second call refunds nothing.
    pub(crate) fn cancel_breeding_visit(&mut self, bay: Entity, slot: usize, reason: CancelReason) {
        let pending = self.pending_visit_slots(bay).contains(&slot);
        let parents = self.visit_parents(bay, slot);
        let names: Vec<String> = parents.iter().map(|&p| self.creature_label(p)).collect();
        for &parent in &parents {
            self.world.entity_mut(parent).remove::<BreedingVisit>();
        }
        if !pending {
            return;
        }
        if let Some(mut incubator) = self.world.get_mut::<Incubator>(bay) {
            incubator.slots[slot] = None;
        }
        let player = self.player_entity();
        if let Some(mut pack) = self.world.get_mut::<Inventory>(player) {
            pack.add(ItemId::from(BREEDING_SEED), 1);
        }
        let who = if names.is_empty() {
            String::new()
        } else {
            format!("{}: ", names.join(" and "))
        };
        self.log_base(format!(
            "{who}The breeding visit is called off ({}). The Breeding Seed is back in your pack.",
            reason.phrase()
        ));
    }

    /// Cancels every pending slot of `bay` — both destruction paths' call,
    /// made before the bay entity goes.
    pub(crate) fn cancel_breeding_visits_at(&mut self, bay: Entity, reason: CancelReason) {
        for slot in self.pending_visit_slots(bay) {
            self.cancel_breeding_visit(bay, slot, reason);
        }
    }
}
