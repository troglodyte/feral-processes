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
use crate::components::Disgruntled;
use crate::tuning::{MORALE_DOWNS_TOOLS_AT, MORALE_RECOVERED_AT};
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
            let marked = self.world.get::<Disgruntled>(worker).is_some();
            // Asymmetric on purpose, and the asymmetry *is* the hysteresis:
            // the entry test is only asked of a body still working, and the
            // exit test only of one that has already stopped. A single
            // comparison against one number is the bug this shape exists to
            // make unwritable.
            if marked {
                if morale >= MORALE_RECOVERED_AT {
                    self.world.entity_mut(worker).remove::<Disgruntled>();
                }
            } else if morale <= MORALE_DOWNS_TOOLS_AT {
                self.world.entity_mut(worker).insert(Disgruntled);
            }
        }
    }
}
