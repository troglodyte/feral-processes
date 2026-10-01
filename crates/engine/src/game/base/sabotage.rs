//! A sulking program spoiling a unit of a machine's output, where others can
//! see it. Spec section 5 of the sulking-behaviours design.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::base_ledger::ConsumeSource;
use crate::components::{MemorySubject, Position, ProgramId, Stock, Structure, Task};
use crate::derive::{FNV_BASIS, fold, index, unit};
use crate::game::base::hauling;
use crate::resources::GameClock;
use crate::situations::chebyshev;
use crate::tuning::{
    BOND_WITNESS_REACH, INTERACTION_PERIOD, SABOTAGE_CHANCE, SABOTAGE_REACH, SABOTAGE_SALT,
};

/// The seed one saboteur's choices at one tick fold from. A function rather
/// than an inline fold so a test can ask which ticks roll under
/// `SABOTAGE_CHANCE` by calling it instead of copying it.
pub(crate) fn sabotage_seed(now: u64, id: ProgramId) -> u64 {
    fold(FNV_BASIS, &[now, id.0 as u64, SABOTAGE_SALT])
}

impl Game {
    /// For each idle sulker in `ProgramId` order, maybe spoils one unit of
    /// the output of a machine it resents or whose operator it avoids, and
    /// tells the staff nearby.
    ///
    /// **No `GameRng`**: every choice is a fold of the tick and the program.
    /// **Candidates are built per body against live stock**, so a second
    /// sulker sees what the first left and never asks `take_from` for a unit
    /// that is gone. Posting, carrying and power are untouched.
    pub(crate) fn note_sabotage(&mut self) {
        let now = self.world.resource::<GameClock>().tick;
        if !now.is_multiple_of(INTERACTION_PERIOD) || !self.base_is_established() {
            return;
        }
        let mut sulkers: Vec<(Entity, ProgramId, Position)> = self
            .base_staff()
            .into_iter()
            .filter(|&e| self.sulks(e) && self.world.get::<Task>(e).is_none())
            .filter_map(|e| {
                Some((
                    e,
                    *self.world.get::<ProgramId>(e)?,
                    *self.world.get::<Position>(e)?,
                ))
            })
            .collect();
        sulkers.sort_by_key(|&(_, id, _)| id);
        for (body, id, at) in sulkers {
            let seed = sabotage_seed(now, id);
            if unit(seed) >= SABOTAGE_CHANCE {
                continue;
            }
            let candidates = self.sabotage_candidates(body, at);
            if candidates.is_empty() {
                continue;
            }
            let machine = candidates[index(fold(seed, &[1, SABOTAGE_SALT]), candidates.len())];
            self.spoil_one(body, id, machine, fold(seed, &[2, SABOTAGE_SALT]));
        }
    }

    /// Machines in reach of `at` holding output, that `body` resents or that
    /// someone it avoids is posted at. Entity order.
    fn sabotage_candidates(&self, body: Entity, at: Position) -> Vec<Entity> {
        let staff = self.base_staff();
        let mut found: Vec<Entity> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let pos = *e.get::<Position>()?;
                let stock = e.get::<Stock>()?;
                if stock.output.is_empty() || chebyshev(pos, at) > SABOTAGE_REACH {
                    return None;
                }
                let rival_worked = staff.iter().any(|&w| {
                    w != body
                        && self
                            .world
                            .get::<Task>(w)
                            .is_some_and(|t| t.target == e.id())
                        && self
                            .world
                            .get::<ProgramId>(w)
                            .is_some_and(|&p| self.bond(body, p).avoids())
                });
                (self.resents_structure(body, kind) || rival_worked).then_some(e.id())
            })
            .collect();
        found.sort();
        found
    }

    /// Takes one unit and tells the story. Silent if the unit is not there.
    fn spoil_one(&mut self, saboteur: Entity, id: ProgramId, machine: Entity, pick: u64) {
        let Some(stock) = self.world.get::<Stock>(machine) else {
            return;
        };
        let held: Vec<_> = stock.output.keys().cloned().collect();
        if held.is_empty() {
            return;
        }
        let item = held[index(pick, held.len())].clone();
        let Some(mut stock) = self.world.get_mut::<Stock>(machine) else {
            return;
        };
        let taken = hauling::take_from(&mut stock, &item, 1);
        if taken == 0 {
            return;
        }
        self.note_consumed(&item, taken, ConsumeSource::Sabotage);
        let kind = self
            .world
            .get::<Structure>(machine)
            .map(|s| s.kind.clone())
            .unwrap_or_default();
        let line = format!(
            "{} spoiled a {} at the {}.",
            self.creature_label(saboteur),
            self.item_name(&item),
            self.structure_name(&kind),
        );
        self.log_base(line);
        let Some(at) = self.world.get::<Position>(machine).copied() else {
            return;
        };
        for witness in self.base_staff() {
            if witness == saboteur {
                continue;
            }
            let near = self
                .world
                .get::<Position>(witness)
                .is_some_and(|&p| chebyshev(p, at) <= BOND_WITNESS_REACH);
            if near {
                self.remember(witness, "saw_sabotage", MemorySubject::Program(id));
            }
        }
    }
}
