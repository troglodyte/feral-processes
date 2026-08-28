//! When a program stops working because of how it feels about the place.
//!
//! `OffShift`'s gate on a different meter, and deliberately built to its
//! shape rather than to a new one. A need runs down and something in the base
//! services it; morale has no amenity to walk to, so what recovers it is the
//! grudges decaying and better memories landing on top — which is to say
//! **time and a base worth working in**, not an errand.
//!
//! **The one thing that must not lapse here is the hysteresis.** In below
//! `MORALE_DOWNS_TOOLS_AT`, out at `MORALE_RECOVERED_AT`, and the gap between
//! them is the feature. Read off the current value alone, a body downs tools
//! and picks them up again on alternate ticks at the boundary — the same
//! flicker `components::OffShift` exists to stop, and the reason both are
//! stored rather than derived.

use crate::Game;
use crate::components::TaskKind;
use crate::components::{Disgruntled, Grievance};
use crate::tuning::{MORALE_DOWNS_TOOLS_AT, MORALE_RECOVERED_AT, MORALE_SULKS_AT};
use bevy_ecs::prelude::Entity;

impl Game {
    /// Inserts, keeps or removes `Disgruntled` for each of `staff`.
    ///
    /// Run beside `update_off_shift`, before the posting half of
    /// `schedule_base_labour` reads `on_shift` — a body that downs tools this
    /// tick must not also be given a job this tick.
    ///
    /// **`morale` and not `opinion_of`.** This is a claim about the body, not
    /// about any one machine or tile: a program that has had a bad run
    /// everywhere is what acting out is, and a program that resents one
    /// machine has always been `drift_idle_staff`'s avoidance rule instead.
    /// The mirror of the choice the parking hook made, one level up.
    ///
    /// It draws no RNG, writes no log line and touches no `Task`. The
    /// standdown is `schedule_base_labour`'s, through the `on_shift` filter,
    /// which already frees a body and already reports the shortfall through
    /// `LabourDemand` — teaching a second function to do either would give
    /// the same state two writers.
    pub(crate) fn update_disgruntled(&mut self, staff: &[Entity]) {
        for &worker in staff {
            let morale = self.morale(worker);
            let marked = self.world.get::<Disgruntled>(worker).map(|d| d.grievance);
            // Asymmetric on purpose, and the asymmetry *is* the hysteresis:
            // the entry test is only asked of a body still working, and the
            // exit test only of one that has already stopped. A single
            // comparison against one number is the bug this shape exists to
            // make unwritable.
            match marked {
                Some(held) => {
                    if morale >= MORALE_RECOVERED_AT {
                        self.world.entity_mut(worker).remove::<Disgruntled>();
                    } else if let Some(now) = reached(morale)
                        && now > held
                    {
                        // **Ratchets, never eases.** Severity only ever
                        // climbs while the marker is held; a body comes back
                        // by recovering, not by wobbling up across the inner
                        // line. Easing here would give the boundary between
                        // the two rungs its own flicker — post, unpost, post
                        // — restarting a cronjob's progress every other tick,
                        // which is the anti-thrash rule this gate sits above.
                        self.world
                            .entity_mut(worker)
                            .insert(Disgruntled { grievance: now });
                    }
                }
                None => {
                    if let Some(grievance) = reached(morale) {
                        self.world
                            .entity_mut(worker)
                            .insert(Disgruntled { grievance });
                    }
                }
            }
        }
    }
}

impl Game {
    /// Whether this program has stopped taking postings altogether — the
    /// severe rung, and the only one the `on_shift` filter cares about.
    ///
    /// A sulking program is still in the pool: it works, just not
    /// everywhere. Reading the marker's presence instead of its severity is
    /// what would collapse the ladder back to one rung.
    pub(crate) fn has_downed_tools(&self, who: Entity) -> bool {
        self.world
            .get::<Disgruntled>(who)
            .is_some_and(|d| d.grievance == Grievance::DownedTools)
    }

    /// Whether `worker` refuses to be posted to `post`.
    ///
    /// Only a sulking body refuses anything, and only a machine it holds a
    /// grudge against — `MEMORY_AVOIDANCE_THRESHOLD` against the structure's
    /// **kind**, which is the subject a `Structure` memory names. The same
    /// constant and the same comparison `drift_idle_staff` declines a tile
    /// on, so a program will not be posted somewhere it would not even stand.
    ///
    /// **Signed, so a fondness can never trigger a refusal**, which is the
    /// rule the parking hook states one level up.
    ///
    /// A `DigSite` is not a structure and has no kind to resent, so an
    /// `Excavate` want is never refused here — the arm skips structurally
    /// rather than by a check.
    pub(crate) fn refuses_post(&self, worker: Entity, post: Entity, kind: TaskKind) -> bool {
        if kind == TaskKind::Excavate {
            return false;
        }
        if self
            .world
            .get::<Disgruntled>(worker)
            .is_none_or(|d| d.grievance != Grievance::Sulking)
        {
            return false;
        }
        let Some(structure) = self.world.get::<crate::components::Structure>(post) else {
            return false;
        };
        let subject = crate::components::MemorySubject::Structure(structure.kind.clone());
        self.opinion_of(worker, &subject) < crate::tuning::MEMORY_AVOIDANCE_THRESHOLD
    }

    /// The index in `idle` of the last body willing to take `post`, or `None`
    /// if every one of them refuses it.
    ///
    /// Scanned from the end so the deepest-first order the caller built is
    /// preserved for everyone who is willing — a sulking body is stepped
    /// over, not promoted past.
    pub(crate) fn willing_index(
        &self,
        idle: &[Entity],
        post: Entity,
        kind: TaskKind,
    ) -> Option<usize> {
        (0..idle.len())
            .rev()
            .find(|&i| !self.refuses_post(idle[i], post, kind))
    }
}

/// Which rung `morale` has reached, or `None` for a program still content.
///
/// The entry side of the gate only — the exit is a single comparison against
/// `MORALE_RECOVERED_AT`, which is what keeps the whole ladder to one
/// hysteresis gap rather than one per rung.
fn reached(morale: f32) -> Option<Grievance> {
    if morale <= MORALE_DOWNS_TOOLS_AT {
        Some(Grievance::DownedTools)
    } else if morale <= MORALE_SULKS_AT {
        Some(Grievance::Sulking)
    } else {
        None
    }
}
